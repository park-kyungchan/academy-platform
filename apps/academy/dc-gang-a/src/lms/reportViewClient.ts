/** The read-only report views of `ReportViewServlet` for the official StudyQ reports `diag`,
 * `new_diag`, `formative`, `ganga`, `na_score` and `smart_book` (specs/academy-mcp/direct-key-lookup.md R8,
 * specs/academy-mcp/pc-executor.md).
 *
 * Request: `POST /servlet/controller.common.ReportViewServlet` with a form body whose first member
 * is `p_process=<View>`, the form of `StudyLink.exe` (0x4692e0):
 * - `DiagReportView`, `FaView`, `GaReportView`, `NaScoreResultView`: `p_process=<View>&p_testing_no=<testingNo>&p_pri_no=<studentKey>&p_cls_no=<classNo>&p_limit_date=<limitDate>`
 *   (`p_limit_date` is empty when no date is given);
 * - `NewStudentReportView`: `p_process=NewStudentReportView&p_p_no=<paperNo>&p_apply_code=<applyCode>&clg_no=<collegeKey>`;
 * - `SmartBookReportView`: `p_process=SmartBookReportView&book_code=<bookCode>&pri_no=<studentKey>`, the book code
 *   `^[A-Za-z0-9._-]{1,64}$` checked before any request.
 * The view name is validated before anything else, and a view without a form here refuses before any
 * request. The answer is an envelope `<body>SERVER_RET = OK<BR>{...json...}<END_DATA></body>`, or the
 * JSON right after `SERVER_RET = OK`; a login or session page refuses with `authentication_required`,
 * any other page with `report_view_unqualified`.
 *
 * The JSON object of the answer is passed verbatim to `StudyQReport.exe` on the PC
 * (specs/academy-mcp/pc-executor.md); nothing here projects it. No error here holds a student name,
 * a record text or a key value. */
import {readInputRecord} from '../contracts/readInputRecord';
import {isLoginOrSessionPage} from './courseBookReport';
import {validateReportViewName,type ReportViewName} from './reportViewRead';

export const REPORT_VIEW_PATH='/servlet/controller.common.ReportViewServlet' as const;
const key=/^[1-9][0-9]{0,15}$/;
const directKey=(value:unknown):string=>{
  if(typeof value!=='string'||!key.test(value)||Number(value)>Number.MAX_SAFE_INTEGER)throw Error('report_view_params_invalid');
  return value;
};
/** `p_cls_no`: 0 (a result of a diagnostic, formative or NA test carries class 0) or a positive key; the key rule otherwise. */
const classKey=(value:unknown):string=>value==='0'?'0':directKey(value);
const limitDate=/^20\d{2}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$/;
/** The book code of `SmartBookReportView`. */
export const SMART_BOOK_CODE=/^[A-Za-z0-9._-]{1,64}$/;

export interface StudentReportViewParams {testingNo:string;studentKey:string;classNo:string;limitDate?:string}
export interface NewStudentReportViewParams {paperNo:string;applyCode:string;collegeKey:string}
export interface SmartBookReportViewParams {bookCode:string;studentKey:string}

/** The exact form body of one view; refuses an unknown view (`report_view_name_invalid`), a view with no
 * form here (`report_view_form_unavailable`) and a malformed parameter (`report_view_params_invalid`). */
export function reportViewForm(view:unknown,params:unknown):URLSearchParams{
  const name:ReportViewName=validateReportViewName(view);
  let data:Record<string,unknown>;
  try{data=readInputRecord(params);}catch{throw Error('report_view_params_invalid');}
  const exact=(names:string[])=>{
    if(Object.keys(data).some(k=>!names.includes(k)))throw Error('report_view_params_invalid');
  };
  if(name==='DiagReportView'||name==='FaView'||name==='GaReportView'||name==='NaScoreResultView'){
    exact(['testingNo','studentKey','classNo','limitDate']);
    const date=data.limitDate===undefined?'':data.limitDate;
    if(typeof date!=='string'||(date!==''&&!limitDate.test(date)))throw Error('report_view_params_invalid');
    return new URLSearchParams([['p_process',name],['p_testing_no',directKey(data.testingNo)],['p_pri_no',directKey(data.studentKey)],
      ['p_cls_no',classKey(data.classNo)],['p_limit_date',date]]);
  }
  if(name==='NewStudentReportView'){
    exact(['paperNo','applyCode','collegeKey']);
    const code=data.applyCode;
    if(typeof code!=='string'||code.length<1||code.length>64||code.trim()!==code||/[\u0000-\u001f\u007f]/.test(code))throw Error('report_view_params_invalid');
    return new URLSearchParams([['p_process',name],['p_p_no',directKey(data.paperNo)],['p_apply_code',code],['clg_no',directKey(data.collegeKey)]]);
  }
  if(name==='SmartBookReportView'){
    exact(['bookCode','studentKey']);
    const code=data.bookCode;
    if(typeof code!=='string'||!SMART_BOOK_CODE.test(code))throw Error('report_view_params_invalid');
    return new URLSearchParams([['p_process',name],['book_code',code],['pri_no',directKey(data.studentKey)]]);
  }
  throw Error('report_view_form_unavailable');
}

/** The JSON object of a `SERVER_RET = OK` answer. */
export function readReportViewEnvelope(text:string):Record<string,unknown>{
  if(isLoginOrSessionPage(text))throw Error('authentication_required');
  const match=/SERVER_RET\s*=\s*OK\s*(?:<BR\s*\/?>)?\s*([\s\S]*?)\s*(?:<END_DATA>|<\/body>|$)/i.exec(text);
  if(!match||!match[1])throw Error('report_view_unqualified');
  try{return readInputRecord(JSON.parse(match[1]));}
  catch{throw Error('report_view_unqualified');}
}
