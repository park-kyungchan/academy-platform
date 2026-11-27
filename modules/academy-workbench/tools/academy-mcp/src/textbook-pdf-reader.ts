import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {open,realpath} from 'node:fs/promises';
import {constants} from 'node:fs';
import {createHash} from 'node:crypto';
import {z} from 'zod';
import {readPinnedLocalBytes} from './item-label-store.ts';

const code=z.string().regex(/^[1-9]\d{0,2}-[1-9]\d{0,2}$/);
export const textbookPageRequest=z.object({
  pages:z.array(z.number().int().min(1)).min(1).max(8).refine(v=>new Set(v).size===v.length).optional(),
  unitCodes:z.array(code).max(16).refine(v=>new Set(v).size===v.length).default([]),
  scanPages:z.number().int().min(1).max(24).default(16),
}).strict();
const label=z.string().min(1).max(2000).refine(s=>!/[\u0000-\u0008\u000b-\u001f\u007f]/.test(s));
const resultSchema=z.object({schemaVersion:z.literal(1),pdfSha256:z.string().regex(/^[a-f0-9]{64}$/),pageCount:z.number().int().positive(),
  scannedPages:z.array(z.number().int().positive()).max(24),
  units:z.array(z.object({code,title:label,majorTitle:label.nullable(),middleTitle:label.nullable(),printedPage:z.number().int().positive().nullable(),pdfPage:z.number().int().positive(),qualification:z.enum(['text_verified','ambiguous_text'])}).strict()).max(512),
  missingUnitCodes:z.array(code).max(16),coverage:z.literal('selected_pages_only'),requiresVisualVerification:z.boolean(),
  reasonCodes:z.array(z.string().regex(/^[a-z_]{1,80}$/)).max(32),textCandidates:z.array(z.object({pdfPage:z.number().int().positive(),text:z.string().max(3000)}).strict()).max(24),
}).strict();
export type TextbookPageRequest=z.infer<typeof textbookPageRequest>;
export type TextbookPdfReading=z.infer<typeof resultSchema>;
export interface TextbookPdfReader{read(path:string,sha256:string,request:TextbookPageRequest):Promise<TextbookPdfReading>}

/** The interpreter at `path` is a regular file, not group or world writable, not reached through a
 * symbolic link, whose bytes hash to `expected`; `code` is the refusal. */
export async function verifyInterpreter(path:string,expected:string,code='textbook_reader_interpreter_rejected'){
  if(!/^[a-f0-9]{64}$/.test(expected)||await realpath(path)!==path)throw Error(code);
  const file=await open(path,constants.O_RDONLY|constants.O_NOFOLLOW|constants.O_NONBLOCK);
  try{
    const before=await file.stat({bigint:true});
    if(!before.isFile()||before.size>32n*1024n*1024n||(before.mode&0o022n)!==0n)throw Error(code);
    const buffer=Buffer.alloc(Number(before.size)+1),{bytesRead}=await file.read(buffer,0,buffer.length,0),after=await file.stat({bigint:true});
    if(BigInt(bytesRead)!==before.size||before.mtimeNs!==after.mtimeNs||before.ctimeNs!==after.ctimeNs||before.size!==after.size
      ||createHash('sha256').update(buffer.subarray(0,bytesRead)).digest('hex')!==expected)throw Error(code);
  }finally{await file.close();}
}

/** Launch-owned interpreter/helper only; tool callers supply no executable/path. */
export async function textbookPdfReaderConfiguration(args:readonly string[]){
  const remainingArgs:string[]=[];let reader:TextbookPdfReader|null=null;
  for(let i=0;i<args.length;i++){
    if(args[i]!=='--textbook-pdf-reader'){remainingArgs.push(args[i]!);continue;}
    if(reader||i+4>=args.length)throw Error('textbook_reader_arguments_invalid');
    const [interpreter,interpreterSha,helper,helperSha]=args.slice(i+1,i+5);
    if(!interpreter?.startsWith('/')||!helper?.startsWith('/'))throw Error('textbook_reader_arguments_invalid');
    await readPinnedLocalBytes(helper!,helperSha!);
    await verifyInterpreter(interpreter!,interpreterSha!);
    reader={async read(path,sha256,request){
      // Reconcile helper drift before each execution; do not inherit Academy credentials.
      const verified=await readPinnedLocalBytes(helper!,helperSha!);
      await verifyInterpreter(interpreter!,interpreterSha!);
      // Execute the verified bytes, not a path reopened after its hash check.
      const program='import base64;exec(compile(base64.b64decode("'+verified.bytes.toString('base64')+'"),"<textbook-pdf-reader>","exec"))';
      let raw:string;
      try{raw=(await promisify(execFile)(interpreter!,['-I','-B','-c',program,path,sha256,JSON.stringify(request)],
        {encoding:'utf8',timeout:15000,maxBuffer:220000,env:{PATH:'/usr/bin:/bin',LANG:'C.UTF-8'}})).stdout;}
      catch{throw Error('textbook_pdf_reader_failed');}
      let value:TextbookPdfReading;
      try{value=resultSchema.parse(JSON.parse(raw));}catch{throw Error('textbook_pdf_reader_result_rejected');}
      const selected=request.pages??Array.from({length:Math.min(request.scanPages,value.pageCount)},(_,n)=>n+1);
      if(value.pdfSha256!==sha256||JSON.stringify(value.scannedPages)!==JSON.stringify(selected)
        ||value.units.some(u=>!selected.includes(u.pdfPage)||(request.unitCodes.length&&!request.unitCodes.includes(u.code)))
        ||value.textCandidates.some(p=>!selected.includes(p.pdfPage)))throw Error('textbook_pdf_reader_result_rejected');
      // Successful structured rows need no full page text in model context.
      if(!value.requiresVisualVerification)value.textCandidates=[];
      else value.textCandidates=value.textCandidates.filter(p=>!value.units.length||value.units.some(u=>u.pdfPage===p.pdfPage&&u.qualification==='ambiguous_text')).slice(0,2);
      return value;
    }};
    i+=4;
  }
  return {remainingArgs,reader};
}
