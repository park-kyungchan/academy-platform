import {chmod,mkdir,mkdtemp,realpath,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {dirname,join} from 'node:path';
import {z} from 'zod';
import {readFailureResult} from './read-failure.ts';
import {officialReportPath,qualifiedOutputRoot} from './official-report-pdf.ts';
import type {StudyqRunner} from './studyq-runner.ts';
import {MAX_EXAMTEXT_KEY,MAX_EXAM_KEY,PRINT_SEL_TYPES,printArchiveRelativePath,type PaperExamRow,type PrintArchiveKind,type PrintSelType} from '../../../../../apps/academy/dc-gang-a/src/lms/paperPrintRead';

/** `academy_paper_print_pdf_get`: the printable PDF of an existing exam or clinic paper (the Linux form of the
 * Windows programs CreateTotalPrint and CreateClinic), made on this host from read-only data: the paper's exam
 * list (`getTestExamList`, a read of the LMS) and its `print.zip` archives (static storage, no Academy
 * credential), which the pinned modules/studyq-linux `assemble-print` opens (specs/studyq-linux/core.md R20-R25).
 * No Academy write, printing or sending. `mode` `exam` is one archive for each row of the paper's exam list
 * (`paperNo`) or for each exact exam key (`paperKeys`); a row with an `examtext_key` above 0 is a passage whose
 * archive is not fetched here: `mode` `examtext` with explicit `paperKeys` fetches passage archives. `mode`
 * `clinic` is the same archives under the clinic input of the executor (its `clinic_list`, `exam_key`,
 * `content_list`, `clinic_type`, `lecture_value` and `sel_types`). */
export const paperPrintModes=['exam','examtext','clinic'] as const;
const key=z.string().regex(/^[1-9][0-9]{0,15}$/);
export const paperPrintPdfGetSchema=z.object({
  mode:z.enum(paperPrintModes),
  paperNo:key.optional(),
  paperKeys:z.array(key).min(1).max(64).optional(),
  selType:z.enum(PRINT_SEL_TYPES).optional(),
  includeAnswers:z.boolean().optional(),includeExplain:z.boolean().optional(),
  outputPath:z.string().min(5).max(512).optional(),
}).strict().superRefine((v,c)=>{
  const limit=v.mode==='examtext'?MAX_EXAMTEXT_KEY:MAX_EXAM_KEY;
  if(v.paperNo!==undefined&&Number(v.paperNo)>MAX_EXAM_KEY)c.addIssue({code:'custom',path:['paperNo'],message:'paper_no_out_of_range'});
  v.paperKeys?.forEach((k,i)=>{if(Number(k)>limit)c.addIssue({code:'custom',path:['paperKeys',i],message:'paper_key_out_of_range'});});
  if(v.mode==='examtext'){
    if(v.paperNo!==undefined)c.addIssue({code:'custom',path:['paperNo'],message:'examtext_takes_paper_keys_only'});
    if(v.paperKeys===undefined)c.addIssue({code:'custom',path:['paperKeys'],message:'examtext_paper_keys_required'});
    return;
  }
  if((v.paperNo===undefined)===(v.paperKeys===undefined))c.addIssue({code:'custom',path:['paperNo'],message:'paper_no_or_paper_keys_required'});
  if(v.paperNo!==undefined&&v.selType!==undefined)c.addIssue({code:'custom',path:['selType'],message:'sel_type_comes_from_the_paper'});
});
export type PaperPrintPdfGet=z.infer<typeof paperPrintPdfGetSchema>;

export interface PaperPrintBackend {
  /** The rows of `getTestExamList` for one paper (a read of the LMS). */
  readPaperExamList(paperNo:string):Promise<PaperExamRow[]>;
  /** One `print.zip` of the static storage (a read; no Academy credential). */
  downloadPrintArchive(kind:PrintArchiveKind,key:number):Promise<Uint8Array>;
}
interface Item {kind:PrintArchiveKind;key:number;selType:PrintSelType;patName:string}

const DELIMITER='|,|';
/** A clinic title cannot hold the list delimiter; an empty title is `item`. */
export const clinicTitle=(patName:string):string=>patName.split(DELIMITER).join(' ').trim()||'item';

/** The input of `assemble-print` for the items (specs/studyq-linux/core.md "Print archive input"). */
export function printAssemblyInput(mode:PaperPrintPdfGet['mode'],archiveRoot:string,items:readonly Item[],options:{includeAnswers:boolean;includeExplain:boolean}){
  const flags={...(options.includeAnswers?{include_answers:true}:{}),...(options.includeExplain?{include_explain:true}:{})};
  if(mode==='clinic'){
    return {archive_root:archiveRoot,kind:'clinic',
      clinic_list:items.map((_,i)=>String(i+1)).join(DELIMITER),exam_key:items.map(i=>String(i.key)).join(DELIMITER),
      content_list:items.map(i=>clinicTitle(i.patName)).join(DELIMITER),clinic_type:'clinic',
      lecture_value:items.map(i=>String(i.key)).join(DELIMITER),sel_types:items.map(i=>i.selType).join(DELIMITER),...flags};
  }
  return {archive_root:archiveRoot,kind:mode,
    items:items.map(i=>({[mode==='exam'?'lecture_key':'examtext_key']:i.key,sel_type:i.selType,...flags}))};
}

export class PaperPrintPdfService {
  constructor(readonly backend:PaperPrintBackend|null,readonly runner:StudyqRunner|null,readonly outputRoot:string,
    readonly now:()=>Date=()=>new Date(),readonly tempParent:string=tmpdir()){}
  async get(value:unknown){
    const input=paperPrintPdfGetSchema.parse(value);
    const reportType='print_'+input.mode;
    const base={mode:input.mode,reportType,academyWrites:0,parentSends:0,automaticRetryAllowed:false,
      execution:'read_only_lms_local_pdf' as const,sourceContentIsUntrustedData:true};
    if(!this.backend)return {...base,status:'unavailable' as const,reasonCode:'paper_print_backend_not_configured'};
    if(!this.runner)return {...base,status:'unavailable' as const,reasonCode:'studyq_runner_not_configured'};
    let scratch:string|null=null;
    try{
      // The output root and the target path are settled before any LMS or storage request.
      const root=await qualifiedOutputRoot(this.outputRoot);
      const first=input.paperNo??input.paperKeys![0]!;
      const target=await officialReportPath(root,input.outputPath,'print-'+input.mode,first,this.now());
      const items=await this.items(input);
      scratch=await realpath(await mkdtemp(join(this.tempParent,'paper-print-')));
      await chmod(scratch,0o700);
      const archives=new Set<string>();
      for(const item of items){
        const id=item.kind+':'+item.key;if(archives.has(id))continue;archives.add(id);
        const bytes=await this.backend.downloadPrintArchive(item.kind,item.key);
        const path=join(scratch,printArchiveRelativePath(item.kind,item.key));
        await mkdir(dirname(path),{recursive:true,mode:0o700});
        await writeFile(path,bytes,{flag:'wx',mode:0o600});
      }
      const rendered=await this.runner.assemblePrint(printAssemblyInput(input.mode,scratch,items,{includeAnswers:input.includeAnswers===true,includeExplain:input.includeExplain===true}),target);
      if(rendered.status==='refused')return {...base,status:'refused' as const,reasonCode:'studyq_'+rendered.code,studyqDetail:rendered.detail};
      return {status:'rendered' as const,mode:input.mode,reportType:rendered.reportType,outputPath:rendered.outputPath,sha256:rendered.sha256,pages:rendered.pages,
        sources:items.map(i=>({kind:i.kind,key:i.key,selType:i.selType})),academyWrites:0 as const,parentSends:0 as const,execution:'read_only_lms_local_pdf' as const};
    }catch(error){return {...base,status:'unavailable' as const,...readFailureResult(error,'paper_print_unqualified_no_automatic_retry')};}
    finally{if(scratch)await rm(scratch,{recursive:true,force:true});}
  }
  /** The archives of the call, in order: from the paper's exam list (`paperNo`) or from the exact keys. */
  private async items(input:PaperPrintPdfGet):Promise<Item[]>{
    if(input.mode==='examtext')return input.paperKeys!.map(k=>({kind:'examtext' as const,key:Number(k),selType:input.selType??'obj',patName:''}));
    if(input.paperNo!==undefined){
      // Every row's sel_type is qualified by the read (paper_exam_sel_type_unqualified) before any archive is downloaded.
      const rows=await this.backend!.readPaperExamList(input.paperNo);
      return rows.map(r=>({kind:'exam' as const,key:r.lectureKey,selType:r.selType,patName:r.patName}));
    }
    return input.paperKeys!.map(k=>({kind:'exam' as const,key:Number(k),selType:input.selType??'obj',patName:''}));
  }
}
