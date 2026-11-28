/** The read-only report views of `ReportViewServlet` for the official StudyQ reports `diag`,
 * `new_diag` and `formative` (specs/academy-mcp/direct-key-lookup.md R8,
 * specs/studyq-linux/reports-batch1.md R4-R6).
 *
 * Request: `POST /servlet/controller.common.ReportViewServlet` with a form body whose first member
 * is `p_process=<View>`, the form of `StudyLink.exe` (0x4692e0):
 * - `DiagReportView`, `FaView`: `p_process=<View>&p_testing_no=<testingNo>&p_pri_no=<studentKey>&p_cls_no=<classNo>&p_limit_date=<limitDate>`
 *   (`p_limit_date` is empty when no date is given);
 * - `NewStudentReportView`: `p_process=NewStudentReportView&p_p_no=<paperNo>&p_apply_code=<applyCode>&clg_no=<collegeKey>`.
 * The view name is validated before anything else, and a view without a form here refuses before any
 * request. The answer is an envelope `<body>SERVER_RET = OK<BR>{...json...}<END_DATA></body>`, or the
 * JSON right after `SERVER_RET = OK`; a login or session page refuses with `authentication_required`,
 * any other page with `report_view_unqualified`.
 *
 * The projections below make the strict inputs of `validate_diag`, `validate_new_diag` and
 * `validate_formative` (modules/studyq-linux) from either form of the payload: the named members of
 * those inputs, or the indexed blocks of the table `REPORT_BLOCKS`. They copy values only; a row is
 * made for each source row and never otherwise, and a value the renderer refuses is left for its
 * strict validator to refuse (input_invalid). No error here holds a student name, a record text or a
 * key value. */
import {readInputRecord} from '../contracts/readInputRecord';
import {isLoginOrSessionPage} from './courseBookReport';
import {validateReportViewName,type ReportViewName} from './reportViewRead';

export const REPORT_VIEW_PATH='/servlet/controller.common.ReportViewServlet' as const;
const key=/^[1-9][0-9]{0,15}$/;
const directKey=(value:unknown):string=>{
  if(typeof value!=='string'||!key.test(value)||Number(value)>Number.MAX_SAFE_INTEGER)throw Error('report_view_params_invalid');
  return value;
};
const limitDate=/^20\d{2}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$/;

export interface StudentReportViewParams {testingNo:string;studentKey:string;classNo:string;limitDate?:string}
export interface NewStudentReportViewParams {paperNo:string;applyCode:string;collegeKey:string}

/** The exact form body of one view; refuses an unknown view (`report_view_name_invalid`), a view with no
 * form here (`report_view_form_unavailable`) and a malformed parameter (`report_view_params_invalid`). */
export function reportViewForm(view:unknown,params:unknown):URLSearchParams{
  const name:ReportViewName=validateReportViewName(view);
  let data:Record<string,unknown>;
  try{data=readInputRecord(params);}catch{throw Error('report_view_params_invalid');}
  const exact=(names:string[])=>{
    if(Object.keys(data).some(k=>!names.includes(k)))throw Error('report_view_params_invalid');
  };
  if(name==='DiagReportView'||name==='FaView'){
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

/** The indexed form of a payload: `block1[0]` holds the header, `block2` one row per area (or unit),
 * `block3` the four levels in the order top, high, mid, low, `block4` one row per question. The
 * numbers are the column keys of each row. */
export const REPORT_BLOCKS={
  diag:{header:{title:'1',test_date:'2',student:'3',grade:'4',total_score:'5',avg_score:'6',comment:'7'},
    rows:{name:'1',score:'2',average:'3',signal:'4',rating:'5'},
    level:{questions:'1',score:'2'},question:{no:'1',area:'2',difficulty:'3',correct_rate:'4',correct:'5'}},
  new_diag:{header:{apply_code:'8',mbti:'9'}},
  formative:{header:{title:'1',test_date:'2',student:'3',my_score:'4',avg_score:'5',rank_signal:'6',comment:'7'},
    rows:{name:'1',my_score:'2',average:'3'},
    level:{questions:'1',score:'2'},question:{no:'1',unit:'2',difficulty:'3',correct_rate:'4',correct:'5'}},
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
