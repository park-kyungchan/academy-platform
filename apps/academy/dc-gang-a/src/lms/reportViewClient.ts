/** The read-only report views of `ReportViewServlet` for the official StudyQ reports `diag`,
 * `new_diag`, `formative`, `ganga`, `na_score` and `smart_book` (specs/academy-mcp/direct-key-lookup.md R8,
 * specs/studyq-linux/reports-batch1.md R4-R6, specs/studyq-linux/reports-batch2.md R4-R6).
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
 * The projections below make the strict inputs of `validate_diag`, `validate_new_diag`,
 * `validate_formative`, `validate_ganga`, `validate_na_score` and `validate_smart_book` (modules/studyq-linux)
 * from either form of the payload: the named members of
 * those inputs, or the indexed blocks of the table `REPORT_BLOCKS`. They copy values only; a row is
 * made for each source row and never otherwise, and a value the renderer refuses is left for its
 * strict validator to refuse (input_invalid). No error here holds a student name, a record text or a
 * key value. */
import {readInputRecord} from '../contracts/readInputRecord';
import {COURSE_BOOK_DIFFICULTIES,COURSE_BOOK_QUESTION_TYPES,isLoginOrSessionPage,type StudyqCourseBookInput} from './courseBookReport';
import {validateReportViewName,type ReportViewName} from './reportViewRead';

export const REPORT_VIEW_PATH='/servlet/controller.common.ReportViewServlet' as const;
const key=/^[1-9][0-9]{0,15}$/;
const directKey=(value:unknown):string=>{
  if(typeof value!=='string'||!key.test(value)||Number(value)>Number.MAX_SAFE_INTEGER)throw Error('report_view_params_invalid');
  return value;
};
const limitDate=/^20\d{2}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$/;
/** The book code of `SmartBookReportView` (and the `book_code` of `validate_smart_book`). */
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
      ['p_cls_no',directKey(data.classNo)],['p_limit_date',date]]);
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

export const REPORT_LEVELS=['top','high','mid','low'] as const;
type Level=typeof REPORT_LEVELS[number];
export interface StudyqDiagInput {
  title:string;student:string;test_date:string;grade:number;total_score:number;avg_score:number;comment:string;
  areas:{name:string;score:number;average:number;signal:string;rating:string}[];
  difficulty:Record<Level,{questions:number;score:number}>;
  questions:{no:number;area:string;difficulty:string;correct_rate:number;correct:unknown}[];
}
export interface StudyqNewDiagInput extends StudyqDiagInput {apply_code:string;mbti:string}
export interface StudyqFormativeInput {
  title:string;student:string;test_date:string;my_score:number;avg_score:number;rank_signal:number;comment:string;
  units:{name:string;my_score:number;average:number}[];
  difficulty:Record<Level,{questions:number;score:number}>;
  questions:{no:number;unit:string;difficulty:string;correct_rate:number;correct:unknown}[];
}

export interface StudyqGangaInput extends StudyqFormativeInput {pyramid_tier:number}
export interface StudyqNaScoreInput {
  title:string;student:string;test_date:string;my_score:number;avg_score:number;rank_band:number;comment:string;
  units:{name:string;my_score:number;average:number}[];
  questions:{no:number;unit:string;difficulty:string;correct_rate:number;correct:unknown}[];
}
export interface StudyqSmartBookInput extends Omit<StudyqCourseBookInput,'bunit_list'> {
  bunit_list:{name:string;my_score:number;average:number;questions:{no:number;type:string;question_type:string;difficulty:string;correct_rate:number;correct:unknown}[]}[];
  book_code:string;
}

/** The indexed form of a payload: `block1[0]` holds the header, `block2` one row per area (or unit),
 * `block3` the four levels in the order top, high, mid, low, `block4` one row per question; `na_score`
 * has no level block and its questions are `block3`. The numbers are the column keys of each row.
 * `smart_book` has the blocks of the course-book report (`SMART_BOOK_BLOCKS`). */
export const REPORT_BLOCKS={
  diag:{header:{title:'1',test_date:'2',student:'3',grade:'4',total_score:'5',avg_score:'6',comment:'7'},
    rows:{name:'1',score:'2',average:'3',signal:'4',rating:'5'},
    level:{questions:'1',score:'2'},question:{no:'1',area:'2',difficulty:'3',correct_rate:'4',correct:'5'}},
  new_diag:{header:{apply_code:'8',mbti:'9'}},
  formative:{header:{title:'1',test_date:'2',student:'3',my_score:'4',avg_score:'5',rank_signal:'6',comment:'7'},
    rows:{name:'1',my_score:'2',average:'3'},
    level:{questions:'1',score:'2'},question:{no:'1',unit:'2',difficulty:'3',correct_rate:'4',correct:'5'}},
  ganga:{header:{title:'1',test_date:'2',student:'3',my_score:'4',avg_score:'5',rank_signal:'6',comment:'7',pyramid_tier:'8'}},
  na_score:{header:{title:'1',test_date:'2',student:'3',my_score:'4',avg_score:'5',rank_band:'6',comment:'7'},
    rows:{name:'1',my_score:'2',average:'3'},question:{no:'1',unit:'2',difficulty:'3',correct_rate:'4',correct:'5'}},
} as const;
/** The block form of `SmartBookReportView`: the blocks of the course-book report (courseBookReport.ts):
 * `block1[0]` `2`, `3` the period's ends and `4` the student; `block2[0]` `1` the grade, `3` and `4` the
 * student's and the overall average, `5` the comment; `block5` the tests; `bunit_list` the chapters
 * with their `exam_result` rows; `block9[0]` `2` to `5` the question counts and `block10` the scores
 * of the four levels; `block12` the units, `block11` their averages. */
export const SMART_BOOK_BLOCKS={
  test:{no:'1',title:'2',questions:'3',average:'4',score:'5',date:'6'},
  chapter:{name:'1',my_score:'3',average:'4'},question:{no:'1',type:'2',difficulty:'3',question_type:'4',correct_rate:'5',correct:'6'},
  munit:{name:'2',correct:'3',total:'4',average:'5'},
} as const;

const record=(value:unknown):Record<string,unknown>=>{try{return readInputRecord(value);}catch{throw Error('report_view_unqualified');}};
const rowsOf=(value:unknown):Record<string,unknown>[]=>{
  if(value===undefined||value===null)return [];
  if(!Array.isArray(value))throw Error('report_view_unqualified');
  return value.map(record);
};
const text=(value:unknown):string=>value===undefined||value===null?'':String(value);
const num=(value:unknown):number=>typeof value==='number'?value:typeof value==='string'&&value.trim()!==''?Number(value):NaN;
/** `O`/`X` of the source become true/false; any other value is passed on for the validator to refuse. */
const mark=(value:unknown):unknown=>value==='O'||value==='o'?true:value==='X'||value==='x'?false:value;
const isBlockForm=(raw:Record<string,unknown>)=>raw.block1!==undefined;

function difficultyOf(raw:Record<string,unknown>,block:boolean,columns:{questions:string;score:string}){
  if(block){
    const rows=rowsOf(raw.block3);
    if(rows.length!==REPORT_LEVELS.length)throw Error('report_view_unqualified');
    return Object.fromEntries(REPORT_LEVELS.map((lv,i)=>[lv,{questions:num(rows[i]![columns.questions]),score:num(rows[i]![columns.score])}])) as Record<Level,{questions:number;score:number}>;
  }
  const levels=record(raw.difficulty);
  return Object.fromEntries(REPORT_LEVELS.map(lv=>{const row=record(levels[lv]);return [lv,{questions:num(row.questions),score:num(row.score)}];})) as Record<Level,{questions:number;score:number}>;
}

function headerOf(raw:Record<string,unknown>,columns:Record<string,string>){
  if(!isBlockForm(raw))return raw;
  const head=rowsOf(raw.block1)[0];
  if(!head)throw Error('report_view_unqualified');
  return Object.fromEntries(Object.entries(columns).map(([name,column])=>[name,head[column]]));
}

/** `DiagReportView` to the `diag` input. */
export function projectDiagReport(raw:Record<string,unknown>):StudyqDiagInput{
  const block=isBlockForm(raw),cols=REPORT_BLOCKS.diag,head=headerOf(raw,cols.header);
  const areaRows=block?rowsOf(raw.block2).map(r=>({name:r[cols.rows.name],score:r[cols.rows.score],average:r[cols.rows.average],signal:r[cols.rows.signal],rating:r[cols.rows.rating]})):rowsOf(raw.areas);
  const questionRows=block?rowsOf(raw.block4).map(r=>({no:r[cols.question.no],area:r[cols.question.area],difficulty:r[cols.question.difficulty],correct_rate:r[cols.question.correct_rate],correct:r[cols.question.correct]})):rowsOf(raw.questions);
  return {
    title:text(head.title),student:text(head.student),test_date:text(head.test_date),grade:num(head.grade),
    total_score:num(head.total_score),avg_score:num(head.avg_score),comment:text(head.comment),
    areas:areaRows.map(r=>({name:text(r.name),score:num(r.score),average:num(r.average),signal:text(r.signal),rating:text(r.rating)})),
    difficulty:difficultyOf(raw,block,cols.level),
    questions:questionRows.map(r=>({no:num(r.no),area:text(r.area),difficulty:text(r.difficulty),correct_rate:num(r.correct_rate),correct:mark(r.correct)})),
  };
}

/** `NewStudentReportView` to the `new_diag` input. */
export function projectNewDiagReport(raw:Record<string,unknown>):StudyqNewDiagInput{
  const head=headerOf(raw,REPORT_BLOCKS.new_diag.header);
  return {...projectDiagReport(raw),apply_code:text(head.apply_code),mbti:text(head.mbti)};
}

/** `FaView` to the `formative` input. */
export function projectFormativeReport(raw:Record<string,unknown>):StudyqFormativeInput{
  const block=isBlockForm(raw),cols=REPORT_BLOCKS.formative,head=headerOf(raw,cols.header);
  const unitRows=block?rowsOf(raw.block2).map(r=>({name:r[cols.rows.name],my_score:r[cols.rows.my_score],average:r[cols.rows.average]})):rowsOf(raw.units);
  const questionRows=block?rowsOf(raw.block4).map(r=>({no:r[cols.question.no],unit:r[cols.question.unit],difficulty:r[cols.question.difficulty],correct_rate:r[cols.question.correct_rate],correct:r[cols.question.correct]})):rowsOf(raw.questions);
  return {
    title:text(head.title),student:text(head.student),test_date:text(head.test_date),my_score:num(head.my_score),
    avg_score:num(head.avg_score),rank_signal:num(head.rank_signal),comment:text(head.comment),
    units:unitRows.map(r=>({name:text(r.name),my_score:num(r.my_score),average:num(r.average)})),
    difficulty:difficultyOf(raw,block,cols.level),
    questions:questionRows.map(r=>({no:num(r.no),unit:text(r.unit),difficulty:text(r.difficulty),correct_rate:num(r.correct_rate),correct:mark(r.correct)})),
  };
}

/** `GaReportView` to the `ganga` input: the `formative` projection and `pyramid_tier`. */
export function projectGangaReport(raw:Record<string,unknown>):StudyqGangaInput{
  const head=headerOf(raw,REPORT_BLOCKS.ganga.header);
  return {...projectFormativeReport(raw),pyramid_tier:num(head.pyramid_tier)};
}

/** `NaScoreResultView` to the `na_score` input. */
export function projectNaScoreReport(raw:Record<string,unknown>):StudyqNaScoreInput{
  const block=isBlockForm(raw),cols=REPORT_BLOCKS.na_score,head=headerOf(raw,cols.header);
  const unitRows=block?rowsOf(raw.block2).map(r=>({name:r[cols.rows.name],my_score:r[cols.rows.my_score],average:r[cols.rows.average]})):rowsOf(raw.units);
  const questionRows=block?rowsOf(raw.block3).map(r=>({no:r[cols.question.no],unit:r[cols.question.unit],difficulty:r[cols.question.difficulty],correct_rate:r[cols.question.correct_rate],correct:r[cols.question.correct]})):rowsOf(raw.questions);
  return {
    title:text(head.title),student:text(head.student),test_date:text(head.test_date),my_score:num(head.my_score),
    avg_score:num(head.avg_score),rank_band:num(head.rank_band),comment:text(head.comment),
    units:unitRows.map(r=>({name:text(r.name),my_score:num(r.my_score),average:num(r.average)})),
    questions:questionRows.map(r=>({no:num(r.no),unit:text(r.unit),difficulty:text(r.difficulty),correct_rate:num(r.correct_rate),correct:mark(r.correct)})),
  };
}

/** A coded label of the course-book source (`3` for 상); a label that is already text is kept. */
const label=(codes:Record<number,string>,value:unknown):string=>{const code=typeof value==='number'?value:typeof value==='string'&&/^\d+$/.test(value)?Number(value):NaN;return codes[code]??text(value);};

/** `SmartBookReportView` of the book `bookCode` to the `smart_book` input: the eleven `course_book`
 * members, from the named members or from the blocks of `SMART_BOOK_BLOCKS`, and `book_code`. A payload
 * that echoes another book code refuses with `official_report_scope_conflict`. Nothing is cut: a source
 * beyond the renderer's limits is refused by its validator (input_invalid). */
export function projectSmartBookReport(raw:Record<string,unknown>,bookCode:string):StudyqSmartBookInput{
  const block=isBlockForm(raw);
  const echoed=block?[raw.book_code,rowsOf(raw.block1)[0]?.book_code]:[raw.book_code];
  if(echoed.some(v=>v!==undefined&&v!==null&&String(v)!==bookCode))throw Error('official_report_scope_conflict');
  if(!block){
    const range=record(raw.score_range);
    return {
      period:text(raw.period),student:text(raw.student),grade:num(raw.grade),my_average:num(raw.my_average),all_average:num(raw.all_average),comment:text(raw.comment),
      score_range:{my:num(range.my),average:num(range.average)},
      test_list:rowsOf(raw.test_list).map(r=>({no:num(r.no),title:text(r.title),questions:num(r.questions),average:num(r.average),score:num(r.score),date:text(r.date)})),
      bunit_list:rowsOf(raw.bunit_list).map(c=>({name:text(c.name),my_score:num(c.my_score),average:num(c.average),
        questions:rowsOf(c.questions).map(q=>({no:num(q.no),type:text(q.type),question_type:text(q.question_type),difficulty:text(q.difficulty),correct_rate:num(q.correct_rate),correct:mark(q.correct)}))})),
      difficulty:difficultyOf(raw,false,{questions:'questions',score:'score'}),
      munit_list:rowsOf(raw.munit_list).map(r=>({name:text(r.name),correct:num(r.correct),total:num(r.total),average:num(r.average)})),
      book_code:bookCode,
    };
  }
  const head=rowsOf(raw.block1)[0];
  if(!head)throw Error('report_view_unqualified');
  const b2=rowsOf(raw.block2)[0]??{},b9=rowsOf(raw.block9)[0]??{},b10=rowsOf(raw.block10),b11=rowsOf(raw.block11),cols=SMART_BOOK_BLOCKS;
  if(b10.length!==REPORT_LEVELS.length)throw Error('report_view_unqualified');
  return {
    period:text(head['2'])+' ~ '+text(head['3']),student:text(head['4']),grade:num(b2['1']),my_average:num(b2['3']),all_average:num(b2['4']),comment:text(b2['5']),
    score_range:{my:num(b2['3']),average:num(b2['4'])},
    test_list:rowsOf(raw.block5).map(r=>({no:num(r[cols.test.no]),title:text(r[cols.test.title]),questions:num(r[cols.test.questions]),average:num(r[cols.test.average]),score:num(r[cols.test.score]),date:text(r[cols.test.date])})),
    bunit_list:rowsOf(raw.bunit_list).map(c=>({name:text(c[cols.chapter.name]),my_score:num(c[cols.chapter.my_score]),average:num(c[cols.chapter.average]),
      questions:rowsOf(c.exam_result).map(q=>({no:num(q[cols.question.no]),type:text(q[cols.question.type]).trim(),question_type:label(COURSE_BOOK_QUESTION_TYPES,q[cols.question.question_type]),
        difficulty:label(COURSE_BOOK_DIFFICULTIES,q[cols.question.difficulty]),correct_rate:num(q[cols.question.correct_rate]),correct:mark(q[cols.question.correct])}))})),
    difficulty:Object.fromEntries(REPORT_LEVELS.map((lv,i)=>[lv,{questions:num(b9[String(i+2)]),score:num(b10[i]!['1'])}])) as Record<Level,{questions:number;score:number}>,
    munit_list:rowsOf(raw.block12).map((r,i)=>({name:text(r[cols.munit.name]),correct:num(r[cols.munit.correct]),total:num(r[cols.munit.total]),average:num(b11[i]?.['2']??r[cols.munit.average])})),
    book_code:bookCode,
  };
}
