/** Read side of the paper PDFs of `academy_paper_print_pdf_get` (specs/academy-mcp/pc-paper-executor.md) and of the exam
 * list that the gated exam write reads (specs/academy-mcp/gated-writes.md). Every request here is a read of the LMS.
 *
 * `getTestExamList` is `POST /servlet/controller.common.TestpageSelectExServlet` with the form `p_process=getTestExamList&p_no=<paperNo>`
 * and `p_option_type`: `2` for the rows of the exam write (`paperExamListForm`), or the print case of the page's typeset module
 * (`teacher_paper`, `student_paper`, `answer_paper`) for a print item (`paperPrintSourceForm`); only with a print case does the server
 * answer the members a print item needs (`add_answer`, `add_comment`, `exam_dispose_type`, ...). The score view of one attempt is
 * `getStudyResultSingleTestingSingleUser` with only `score.create` 1 (a read, lmsRouteRegistry.ts testpageSelectExRequestEffect); its
 * `print_list` is passed on as the server answered it. No error of this module holds a key value, a name or a response text. */
import {readInputRecord} from '../contracts/readInputRecord';
import {clinicReadbackFields} from './clinicPaperWrite';

export const PAPER_EXAM_LIST_PATH='/servlet/controller.common.TestpageSelectExServlet' as const;
export const MAX_PAPER_EXAM_LIST_ROWS=200;
export const MAX_EXAM_KEY=999_999_999;
export const MAX_EXAMTEXT_KEY=999_999;
export const PRINT_SEL_TYPES=['obj','sbj','des'] as const;
export type PrintSelType=typeof PRINT_SEL_TYPES[number];
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
 * `paper_exam_sel_type_unqualified`. */
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

/** The print cases of `MODULE.Typeset.createTestPaper` of the page, each with its `paper_type` (`PAPER_TYPE.ETEST` `test` or `PAPER_TYPE.ANSWER` `answer`). */
export const PAPER_PRINT_CASES=['teacher_paper','student_paper','answer_paper'] as const;
export type PaperPrintCase=typeof PAPER_PRINT_CASES[number];
export const PAPER_PRINT_TYPES:Readonly<Record<PaperPrintCase,'test'|'answer'>>={teacher_paper:'test',student_paper:'test',answer_paper:'answer'};

/** The form body of `getTestExamList` for one paper and one print case, as `createTestPaper` posts it (`p_ass_type` is undefined there and so not sent). */
export function paperPrintSourceForm(paperNo:string,printCase:PaperPrintCase):Record<string,string>{
  if(!paperNoQualified(paperNo))throw Error('paper_no_invalid');
  if(!(PAPER_PRINT_CASES as readonly string[]).includes(printCase))throw Error('paper_print_case_invalid');
  return {p_process:'getTestExamList',p_no:paperNo,p_option_type:printCase};
}
const asRecord=(value:unknown):Record<string,unknown>|null=>value!==null&&typeof value==='object'&&!Array.isArray(value)?value as Record<string,unknown>:null;
const answerOf=(text:string):Record<string,unknown>=>{
  let value:unknown;
  try{value=JSON.parse(text);}catch{throw Error('paper_print_source_unverified');}
  const record=asRecord(value);
  if(!record||record.result!=='OK')throw Error('paper_print_source_unverified');
  return record;
};
/** The `getTestExamList` answer of a print case: `result` `OK` and an `exam_list` array of objects; an empty list is `paper_print_list_empty`. */
export function parsePaperPrintSource(text:string):Record<string,unknown>{
  const answer=answerOf(text);
  if(!Array.isArray(answer.exam_list))throw Error('paper_print_source_unverified');
  if(answer.exam_list.length===0)throw Error('paper_print_list_empty');
  if(answer.exam_list.length>MAX_PAPER_EXAM_LIST_ROWS||!answer.exam_list.every(row=>asRecord(row)))throw Error('paper_print_source_unverified');
  return answer;
}
/** The logo of a paper item: the school's logo on the static storage, as the page passes it for this school. */
export const paperLogoUrl=(schoolKey:string)=>{
  if(!/^[1-9][0-9]{0,15}$/.test(schoolKey))throw Error('paper_print_principal_invalid');
  return `https://storage.studyq.net/school_data/${schoolKey}/${schoolKey}-logo.png`;
};
/** The one print item of a paper, member for member as `createTestPaper` and `addTypesetObject` (`getEachModeTypesetObject`, print type
 * `each`) of the page build it from the `getTestExamList` answer: `stage_id` of the first exam row, `add_incorrect_comment` from
 * `add_comment`, `student_name` empty, `create_answer` not passed. A member whose source is absent is absent, as `JSON.stringify` drops it. */
export function paperPrintItem(answer:Record<string,unknown>,printCase:PaperPrintCase,schoolKey:string):Record<string,unknown>{
  const first=asRecord((answer.exam_list as unknown[])[0]);
  const item:Record<string,unknown>={print_type:'each',paper_type:PAPER_PRINT_TYPES[printCase],logo:paperLogoUrl(schoolKey),
    examset_title:answer.examset_title,sub_title:answer.sub_title,examset_type:answer.examset_type,stage_id:first?.stage_id,
    add_incorrect_comment:answer.add_comment,add_answer:answer.add_answer,add_explain:answer.add_explain,add_code:answer.add_code,
    add_difficulty:answer.add_difficulty,add_pat:answer.add_pat,theme_type:answer.theme_type,theme_color:answer.theme_color,
    exam_dispose_type:answer.exam_dispose_type,exam_spacer:answer.exam_spacer,student_name:'',test_code:answer.test_code,exam_list:answer.exam_list};
  return Object.fromEntries(Object.entries(item).filter(([,value])=>value!==undefined));
}

/** The form of the score view of one attempt: exactly the clinic readback's (`clinicReadbackFields`): `student_name` empty, `score.create` 1, nothing created. */
export function resultPrintListForm(testingNo:string,studentKey:string){
  if(!paperNoQualified(testingNo)||!/^[1-9][0-9]{0,15}$/.test(studentKey))throw Error('paper_no_invalid');
  return clinicReadbackFields({studentKey,testingNo});
}
/** The `print_list` of a score view, as the server answered it: `result` `OK`, an array of objects; an item that names another
 * student (`pri_no`) is `paper_print_source_unverified`; an empty list is `paper_print_list_empty`. */
export function parseResultPrintList(text:string,studentKey:string):Record<string,unknown>[]{
  const answer=answerOf(text);
  if(!Array.isArray(answer.print_list))throw Error('paper_print_source_unverified');
  if(answer.print_list.length===0)throw Error('paper_print_list_empty');
  if(answer.print_list.length>MAX_PAPER_EXAM_LIST_ROWS)throw Error('paper_print_source_unverified');
  return answer.print_list.map(raw=>{
    const item=asRecord(raw);
    if(!item||(item.pri_no!==undefined&&String(item.pri_no)!==studentKey))throw Error('paper_print_source_unverified');
    return item;
  });
}
