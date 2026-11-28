/** `getCourseBookReport` of `TextBookManageServlet`: the official course-book report of one
 * student, course and book (specs/academy-mcp/direct-key-lookup.md R3, R4).
 *
 * Request: `POST /servlet/controller.coursemanage.TextBookManageServlet`, form body
 * `reqCmd=getCourseBookReport&course_key=<k>&book_no=<n>&student_pri_no=<s>`. The answer is an
 * envelope `<body>SERVER_RET = OK<BR>{...json...}<END_DATA></body>`; its blocks are indexed by
 * the column numbers of the source (`block1[0]['1']` is the book title). The report is verified
 * against the request and kept as the server answered it (`raw`): the PC executor
 * (specs/academy-mcp/pc-executor.md) passes it verbatim to `StudyQReport.exe`. The few values read
 * here (the book title, the student, the period and the two averages) serve the title check and the
 * report summary only. No error of this module holds a student name, a record text or a key value. */
import {readInputRecord} from '../contracts/readInputRecord';

export const COURSE_BOOK_REPORT_PATH='/servlet/controller.coursemanage.TextBookManageServlet' as const;
export interface CourseBookReport {
  metadata:{bookTitle:string};
  /** The number of chapters (`bunit_list`) the report holds. */
  sourceChapterCount:number;
  /** The JSON object of the answer exactly as the server sent it. */
  raw:Record<string,unknown>;
}

/** A login page, a session-error page or a password form: the session is not (or no longer) the teacher's. */
const sessionPage=/webUserPass|세션이 만료|normal_session_error\.jsp|popup_session_error\.jsp|<input\b[^>]*\btype\s*=\s*["']?password\b|<form\b[^>]*\bname\s*=\s*["']?loginForm\b/i;
export function isLoginOrSessionPage(text:string):boolean {return sessionPage.test(text);}

/** The JSON object of a `SERVER_RET = OK` envelope. A login or session page refuses with
 * `authentication_required`; any other page, or an envelope without `SERVER_RET = OK`, with
 * `historical_key_unverified`. */
export function readCourseBookReportEnvelope(text:string):Record<string,unknown> {
  if(isLoginOrSessionPage(text))throw Error('authentication_required');
  const match=/SERVER_RET\s*=\s*OK\s*<BR\s*\/?>([\s\S]*?)<END_DATA>/i.exec(text);
  if(!match)throw Error('historical_key_unverified');
  try{return readInputRecord(JSON.parse(match[1]!.trim()));}
  catch{throw Error('historical_key_unverified');}
}

const rows=(raw:Record<string,unknown>,name:string):Record<string,unknown>[]=>{
  const value=raw[name];
  if(value===undefined||value===null)return [];
  if(!Array.isArray(value))throw Error('historical_key_unverified');
  return value.map(row=>{try{return readInputRecord(row);}catch{throw Error('historical_key_unverified');}});
};
/** Verify the report against the request (R4). `expected.studentName` must be `block1[0]['4']`;
 * `course_key`, `book_no` and `student_pri_no`, wherever the payload echoes them (top level or
 * `block1[0]`), must be the keys sent. The answer is kept whole. */
export function verifyCourseBookReport(raw:Record<string,unknown>,expected:{studentName:string;studentKey:string;courseKey:string;bookNo:string}):CourseBookReport {
  const block1=rows(raw,'block1'),head=block1[0];
  if(!head)throw Error('historical_key_unverified');
  const bookTitle=String(head['1']??'').trim();
  if(!bookTitle||head['4']!==expected.studentName)throw Error('historical_key_unverified');
  const echoes:[string,string][]=[['course_key',expected.courseKey],['book_no',expected.bookNo],['student_pri_no',expected.studentKey]];
  for(const source of [raw,head])for(const [name,value] of echoes){
    const echoed=source[name];
    if(echoed!==undefined&&echoed!==null&&String(echoed)!==value)throw Error('historical_key_unverified');
  }
  return {metadata:{bookTitle},sourceChapterCount:rows(raw,'bunit_list').length,raw};
}
/** The student (`block1[0]['4']`), the period (`block1[0]['2'] ~ block1[0]['3']`) and the student's and the
 * overall average (`block2[0]['3']`, `block2[0]['4']`) of a raw course-book report. */
export function courseBookSummary(raw:Record<string,unknown>){
  const head=rows(raw,'block1')[0]??{},b2=rows(raw,'block2')[0]??{};
  return {student:String(head['4']??''),period:String(head['2'])+' ~ '+String(head['3']),myAverage:Number(b2['3']),allAverage:Number(b2['4'])};
}
