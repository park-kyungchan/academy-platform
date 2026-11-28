/** `getCourseBookReport` of `TextBookManageServlet`: the official course-book report of one
 * student, course and book (specs/academy-mcp/direct-key-lookup.md R3, R4).
 *
 * Request: `POST /servlet/controller.coursemanage.TextBookManageServlet`, form body
 * `reqCmd=getCourseBookReport&course_key=<k>&book_no=<n>&student_pri_no=<s>`. The answer is an
 * envelope `<body>SERVER_RET = OK<BR>{...json...}<END_DATA></body>`; its blocks are indexed by
 * the column numbers of the source (`block1[0]['1']` is the book title). The projection below
 * pins every index the `course_book` renderer of modules/studyq-linux takes: the eleven members
 * of `validate_course_book` and nothing else. The book title is kept apart as metadata: it is
 * used for the report summary and the title check, never as a renderer input. No error of this
 * module holds a student name, a record text or a key value. */
import {readInputRecord} from '../contracts/readInputRecord';

export const COURSE_BOOK_REPORT_PATH='/servlet/controller.coursemanage.TextBookManageServlet' as const;
/** The limits of modules/studyq-linux/src/studyq_linux/schema.py. */
export const COURSE_BOOK_LIMITS={tests:8,chapters:30,questions:200,munits:11} as const;

export interface StudyqCourseBookInput {
  period:string;student:string;grade:number;my_average:number;all_average:number;comment:string;
  score_range:{my:number;average:number};
  test_list:{no:number;title:string;questions:number;average:number;score:number;date:string}[];
  bunit_list:{name:string;my_score:number;average:number;questions:{no:number;type:string;question_type:string;difficulty:string;correct_rate:number;correct:boolean}[]}[];
  difficulty:Record<'top'|'high'|'mid'|'low',{questions:number;score:number}>;
  munit_list:{name:string;correct:number;total:number;average:number}[];
}
export interface CourseBookTruncation {testsDropped:number;chaptersDropped:number;questionsDropped:number;munitsDropped:number}
export interface CourseBookReport {
  metadata:{bookTitle:string};
  truncation:CourseBookTruncation;
  /** The number of chapters the source holds, before the renderer's limit. */
  sourceChapterCount:number;
  studyqInput:StudyqCourseBookInput;
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
/** The labels of the coded question type and difficulty of the course-book blocks (also those of `SmartBookReportView`). */
export const COURSE_BOOK_QUESTION_TYPES:Readonly<Record<number,string>>={1:'객관식',2:'단답형',3:'서술형'};
export const COURSE_BOOK_DIFFICULTIES:Readonly<Record<number,string>>={1:'하',2:'중',3:'상',4:'최상'};
const questionTypes=COURSE_BOOK_QUESTION_TYPES,difficulties=COURSE_BOOK_DIFFICULTIES;

/** Verify the report against the request (R4) and project it. `expected.studentName` must be
 * `block1[0]['4']`; `course_key`, `book_no` and `student_pri_no`, wherever the payload echoes
 * them (top level or `block1[0]`), must be the keys sent. */
export function projectCourseBookReport(raw:Record<string,unknown>,expected:{studentName:string;studentKey:string;courseKey:string;bookNo:string}):CourseBookReport {
  const block1=rows(raw,'block1'),head=block1[0];
  if(!head)throw Error('historical_key_unverified');
  const bookTitle=String(head['1']??'').trim();
  if(!bookTitle||head['4']!==expected.studentName)throw Error('historical_key_unverified');
  const echoes:[string,string][]=[['course_key',expected.courseKey],['book_no',expected.bookNo],['student_pri_no',expected.studentKey]];
  for(const source of [raw,head])for(const [name,value] of echoes){
    const echoed=source[name];
    if(echoed!==undefined&&echoed!==null&&String(echoed)!==value)throw Error('historical_key_unverified');
  }
  const block2=rows(raw,'block2'),block5=rows(raw,'block5'),block9=rows(raw,'block9'),block10=rows(raw,'block10'),
    block11=rows(raw,'block11'),block12=rows(raw,'block12'),chapters=rows(raw,'bunit_list');
  const b2=block2[0]??{},b9=block9[0]??{};
  const examResult=(chapter:Record<string,unknown>)=>Array.isArray(chapter.exam_result)?chapter.exam_result.map(q=>{try{return readInputRecord(q);}catch{throw Error('historical_key_unverified');}}):[];
  const keptChapters=chapters.slice(0,COURSE_BOOK_LIMITS.chapters);
  const studyqInput:StudyqCourseBookInput={
    period:String(head['2'])+' ~ '+String(head['3']),
    student:String(head['4']),
    grade:Number(b2['1']),
    my_average:Number(b2['3']),
    all_average:Number(b2['4']),
    comment:String(b2['5']??''),
    score_range:{my:Number(b2['3']),average:Number(b2['4'])},
    test_list:block5.slice(0,COURSE_BOOK_LIMITS.tests).map(r=>({no:Number(r['1']),title:String(r['2']),questions:Number(r['3']),average:Number(r['4']),score:Number(r['5']),date:String(r['6'])})),
    bunit_list:keptChapters.map(ch=>({name:String(ch['1']),my_score:Number(ch['3']),average:Number(ch['4']),
      // An empty type is left empty: the renderer refuses it with input_invalid, no label is invented.
      questions:examResult(ch).slice(0,COURSE_BOOK_LIMITS.questions).map(q=>({no:Number(q['1']),type:String(q['2']??'').trim(),
        question_type:questionTypes[Number(q['4'])]??String(q['4']),difficulty:difficulties[Number(q['3'])]??String(q['3']),
        correct_rate:Number(q['5']),correct:q['6']==='O'}))})),
    difficulty:{
      top:{questions:Number(b9['2']),score:Number(block10[0]?.['1'])},
      high:{questions:Number(b9['3']),score:Number(block10[1]?.['1'])},
      mid:{questions:Number(b9['4']),score:Number(block10[2]?.['1'])},
      low:{questions:Number(b9['5']),score:Number(block10[3]?.['1'])},
    },
    munit_list:block12.slice(0,COURSE_BOOK_LIMITS.munits).map((r,i)=>({name:String(r['2']),correct:Number(r['3']),total:Math.max(1,Number(r['4'])),average:Number(block11[i]?.['2']??r['5'])})),
  };
  const truncation={testsDropped:Math.max(0,block5.length-COURSE_BOOK_LIMITS.tests),chaptersDropped:Math.max(0,chapters.length-COURSE_BOOK_LIMITS.chapters),
    questionsDropped:keptChapters.reduce((sum,ch)=>sum+Math.max(0,examResult(ch).length-COURSE_BOOK_LIMITS.questions),0),
    munitsDropped:Math.max(0,block12.length-COURSE_BOOK_LIMITS.munits)};
  return {metadata:{bookTitle},truncation,sourceChapterCount:chapters.length,studyqInput};
}
export const courseBookTruncated=(t:CourseBookTruncation)=>t.testsDropped+t.chaptersDropped+t.questionsDropped+t.munitsDropped>0;
