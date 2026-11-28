/** Read side of the print PDFs of `academy_paper_print_pdf_get` (specs/studyq-linux/core.md R20-R25,
 * specs/academy-mcp/tool-surface.md "New tools"): the paper's exam list from the LMS and the
 * `print.zip` archives from the static storage, both reads.
 *
 * `getTestExamList` is `POST /servlet/controller.common.TestpageSelectExServlet` with the form
 * `p_process=getTestExamList&p_no=<paperNo>&p_option_type=2`; the answer is a JSON object
 * `{result:'OK',exam_list:[{lecture_key,sel_type,examtext_key,pat_name,difficulty,unit_name}]}`.
 * An archive is `GET https://storage.studyq.net/data/contents/exam/d6-%03d/d4-%02d/d2-%02d/ek-%u/%u-print.zip`
 * (or `examtext/d4-%02d/d2-%02d/etk-%u/%u-print.zip`): a static URL made from an integer key only,
 * fetched with no credential and no redirect. The archives are opened by modules/studyq-linux, which
 * opens no network connection. No error of this module holds a key value, a name or a response text. */
import {readInputRecord} from '../contracts/readInputRecord';

export const PAPER_EXAM_LIST_PATH='/servlet/controller.common.TestpageSelectExServlet' as const;
export const PRINT_STORAGE_ORIGIN='https://storage.studyq.net' as const;
export const MAX_PRINT_ARCHIVE_BYTES=16*1024*1024;
export const MAX_PAPER_EXAM_LIST_ROWS=200;
export const MAX_EXAM_KEY=999_999_999;
export const MAX_EXAMTEXT_KEY=999_999;
export const PRINT_SEL_TYPES=['obj','sbj','des'] as const;
export type PrintSelType=typeof PRINT_SEL_TYPES[number];
export type PrintArchiveKind='exam'|'examtext';
/** `outerLectureKey` is the `outer_lecture_key` member of the row (the key the exam creation page stages), null when the member is absent. */
export interface PaperExamRow {lectureKey:number;outerLectureKey:number|null;selType:PrintSelType;examtextKey:number;patName:string;difficulty:string;unitName:string}

/** A paper number is a decimal key, 1 to 999999999. */
export function paperNoQualified(value:unknown):value is string{
  return typeof value==='string'&&/^[1-9][0-9]{0,15}$/.test(value)&&Number(value)<=MAX_EXAM_KEY;
}

/** The form body of `getTestExamList` for one paper. */
export function paperExamListForm(paperNo:string):Record<string,string>{
  if(!paperNoQualified(paperNo))throw Error('paper_no_invalid');
  return {p_process:'getTestExamList',p_no:paperNo,p_option_type:'2'};
}

const keyOf=(value:unknown,max:number):number=>{
  const n=typeof value==='number'?value:typeof value==='string'&&/^(0|[1-9][0-9]{0,8})$/.test(value)?Number(value):NaN;
  if(!Number.isInteger(n)||n<0||n>max)throw Error('paper_exam_list_unverified');
  return n;
};
const textOf=(value:unknown):string=>{
  if(value===undefined||value===null)return '';
  if(typeof value!=='string'&&typeof value!=='number')throw Error('paper_exam_list_unverified');
  return String(value).slice(0,200);
};

/** The rows of a `getTestExamList` answer. `result` must be `OK` and every row's `sel_type` one of
 * `obj`, `sbj`, `des`: a row of another kind refuses the whole list with
 * `paper_exam_sel_type_unqualified`, before any archive is downloaded. */
export function parsePaperExamList(text:string):PaperExamRow[]{
  let data:Record<string,unknown>;
  try{data=readInputRecord(JSON.parse(text));}catch{throw Error('paper_exam_list_unverified');}
  if(data.result!=='OK'||!Array.isArray(data.exam_list))throw Error('paper_exam_list_unverified');
  if(data.exam_list.length===0)throw Error('paper_exam_list_empty');
  if(data.exam_list.length>MAX_PAPER_EXAM_LIST_ROWS)throw Error('paper_exam_list_too_large');
  return data.exam_list.map(raw=>{
    let row:Record<string,unknown>;
    try{row=readInputRecord(raw);}catch{throw Error('paper_exam_list_unverified');}
    const selType=row.sel_type;
    if(typeof selType!=='string'||!(PRINT_SEL_TYPES as readonly string[]).includes(selType))throw Error('paper_exam_sel_type_unqualified');
    return {lectureKey:keyOf(row.lecture_key,MAX_EXAM_KEY),outerLectureKey:row.outer_lecture_key===undefined||row.outer_lecture_key===null?null:keyOf(row.outer_lecture_key,MAX_EXAM_KEY),selType:selType as PrintSelType,
      examtextKey:row.examtext_key===undefined||row.examtext_key===null||row.examtext_key===''?0:keyOf(row.examtext_key,MAX_EXAMTEXT_KEY),
      patName:textOf(row.pat_name),difficulty:textOf(row.difficulty),unitName:textOf(row.unit_name)};
  });
}

const pad=(n:number,width:number)=>String(n).padStart(width,'0');
/** The path of an archive under the storage root: `data/contents/...`, from an integer key only (R21 of studyq-linux/core.md). */
export function printArchiveRelativePath(kind:PrintArchiveKind,key:number):string{
  if(!Number.isInteger(key)||key<0||key>(kind==='exam'?MAX_EXAM_KEY:MAX_EXAMTEXT_KEY))throw Error('print_archive_key_invalid');
  if(kind==='exam'){
    const d6=Math.floor(key/1_000_000),d4=Math.floor((key%1_000_000)/10_000),d2=Math.floor(((key%1_000_000)%10_000)/100);
    return `data/contents/exam/d6-${pad(d6,3)}/d4-${pad(d4,2)}/d2-${pad(d2,2)}/ek-${key}/${key}-print.zip`;
  }
  const d4=Math.floor(key/10_000),d2=Math.floor((key%10_000)/100);
  return `data/contents/examtext/d4-${pad(d4,2)}/d2-${pad(d2,2)}/etk-${key}/${key}-print.zip`;
}
export function printArchiveUrl(kind:PrintArchiveKind,key:number):URL{
  return new URL('/'+printArchiveRelativePath(kind,key),PRINT_STORAGE_ORIGIN);
}

/** The body of an archive answer: HTTP 200, not redirected, not a text page, a ZIP signature and at most
 * `MAX_PRINT_ARCHIVE_BYTES` read (the declared length is checked first, the bytes actually read count). */
export async function boundedPrintArchiveBody(response:Response):Promise<Uint8Array>{
  const reject=async(code:string):Promise<never>=>{try{await response.body?.cancel();}catch{}throw Error(code);};
  if(response.status!==200||response.redirected||response.type==='opaqueredirect')return reject('paper_print_archive_rejected');
  const mime=(response.headers.get('content-type')??'').split(';')[0]!.trim().toLowerCase();
  if(mime.startsWith('text/')||mime==='application/json')return reject('paper_print_archive_rejected');
  const header=response.headers.get('content-length'),size=header===null?null:Number(header);
  if(size!==null&&(!Number.isSafeInteger(size)||size<1||size>MAX_PRINT_ARCHIVE_BYTES))return reject('paper_print_archive_rejected');
  const reader=response.body?.getReader();if(!reader)throw Error('paper_print_archive_rejected');
  const chunks:Uint8Array[]=[];let count=0;
  try{
    for(;;){
      const next=await reader.read();if(next.done)break;
      count+=next.value.byteLength;
      if(count>MAX_PRINT_ARCHIVE_BYTES){await reader.cancel();throw Error('paper_print_archive_rejected');}
      chunks.push(next.value);
    }
  }finally{reader.releaseLock();}
  const encoded=response.headers.get('content-encoding');
  if(!count||(size!==null&&(!encoded||encoded==='identity')&&count!==size))throw Error('paper_print_archive_rejected');
  const bytes=new Uint8Array(count);let offset=0;for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.byteLength;}
  if(bytes[0]!==0x50||bytes[1]!==0x4b||bytes[2]!==0x03||bytes[3]!==0x04)throw Error('paper_print_archive_rejected');
  return bytes;
}
