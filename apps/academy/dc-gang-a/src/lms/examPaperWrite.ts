import {readForm,submittedMembers,xmlLeaves} from './examMarkup';
import {EXAM_LEDGER_TOOL,GatedWriteRefusal,TARGET_STRING,canonicalJson,sha256Hex} from './gatedWriteLedger';
import {selectSourceFunction} from './sourceFunctionText';
import {DIAGNOST_MANAGE_PATH,OBJECT_SELECT_PATH,TESTPAGE_SELECT_PATH,examHangPath,examRegPath,type ExamFamily} from './lmsRouteRegistry';
import {MAX_EXAM_KEY,paperNoQualified} from './paperPrintRead';

/**
 * The wire protocol of the gated FA and unit-exam creation and assignment (specs/academy-mcp/gated-writes.md, `academy_exam_paper_review`
 * and `_save`): the pinned page hashes, the qualification of a review request into one frozen change, the exact requests of the three
 * save steps (A stage the items, B save the paper, C assign it) and the checks of their answers. Pure: no request is sent here (the
 * service of src/exam-write-tools.ts sends them through its port).
 */

/** The ledger family of both tools (R4: a token of the clinic pair is `review_token_invalid` here). */
export const EXAM_TOOL_FAMILY = EXAM_LEDGER_TOOL;

// ---- pinned page hashes ---------------------------------------------------------------------------

export const CREATION_FUNCTIONS = ['validationTestInfo', 'requestSave', 'onSaveButtonClick'] as const;
export const ASSIGNMENT_FUNCTIONS = ['validate', 'onSave', 'showSaveResult'] as const;
/**
 * Pinned production hashes: `selectSourceFunction` of each named function of the page text (line ends normalized to `\n`), in the order
 * listed, joined by one `\n`, SHA-256 of the UTF-8 bytes (the rule of cismWeeklyProtocol.ts). `creation` is the allocating
 * `TestPageRegServlet?p_process=Auto` page, `assignment` the `TestHangServlet?p_process=Main&p_no=<testno>` page.
 */
export const EXAM_PAGE_SHA256 = {
  fa: {creation: '7c1ae5a66888e6d4f5aa0b3089bee77dda8e75a3c3c8a4da3b40b0d0f111df1e', assignment: 'ed6e161327a83e13a305891dcecd1a0afc4c9b42dbc0eead1bf674c7c2cda7f3'},
  da: {creation: '2c1c72d5a81a117afce22dc1f83c460e9db6b79a6adfdfbb26519d574206af3d', assignment: '1fa2e96e224d105d39684bcba248b2b6ae8f054bc7dff8388c81eeb0afc20e67'},
} as const;
export type ExamPage = 'creation' | 'assignment';
/** Test seam: replaces some of the pinned production hashes so synthetic pages can exercise every path. */
export type ExpectedExamWireHashes = {[F in ExamFamily]?: {[P in ExamPage]?: string}};
export const expectedExamPageHash = (family: ExamFamily, page: ExamPage, override?: ExpectedExamWireHashes): string => override?.[family]?.[page] ?? EXAM_PAGE_SHA256[family][page];

/** The hash of a page's write program, or null when a function is absent, ambiguous or cannot be selected. */
export function examPageSha256(page: ExamPage, html: string): string | null {
  try {
    const normalized = html.replaceAll('\r\n', '\n');
    const names = page === 'creation' ? CREATION_FUNCTIONS : ASSIGNMENT_FUNCTIONS;
    const parts = names.map(name => selectSourceFunction(normalized, name));
    return parts.every(part => part) ? sha256Hex(parts.join('\n')) : null;
  } catch { return null; }
}
/** `wireSha256` of the review token: SHA-256 of `<family>:` + the pinned creation hash + `:` + the pinned assignment hash of that family. */
export const examWireSha256 = (family: ExamFamily, override?: ExpectedExamWireHashes): string =>
  sha256Hex(family + ':' + expectedExamPageHash(family, 'creation', override) + ':' + expectedExamPageHash(family, 'assignment', override));

// ---- input qualification --------------------------------------------------------------------------

const KEY = /^[1-9][0-9]{0,14}$/;
const ITEM_KEY = /^[1-9][0-9]{0,8}$/;
const PUBLIC_MEMBER = /public|all|everyone|every|academy|school|wide|global|pool|grade/i;
const CLASS_MEMBER = /class|cls|group/i;
export const MAX_ITEMS = 30;
export const MAX_TARGETS = 40;
export const MAX_TEST_NAME = 50;
export const MAX_STUDENT_NAME = 20;
const REQUEST_MEMBERS = ['family', 'testName', 'testgubunNo', 'sourcePaperNo', 'items', 'courseNo', 'targets', 'start', 'end', 'testingMinutes', 'private_chk'] as const;
const CHANGE_MEMBERS = ['steps', 'family', 'testName', 'testgubunNo', 'sourcePaperNo', 'items', 'courseNo', 'targets', 'start', 'end', 'testingMinutes', 'testno', 'munhangCount', 'private_chk'] as const;
const TARGET_MEMBERS = ['studentName', 'studentKey', 'iden'] as const;
const CHANGE_TARGET_MEMBERS = ['studentName', 'studentKey', 'gradeNo', 'clsNo'] as const;
export const ITEM_TYPES = ['obj', 'sbj', 'des'] as const;
export type ExamItemType = typeof ITEM_TYPES[number];
export interface ExamItem {key: string; type: ExamItemType}
/** A target of a change: one student (iden 1); `gradeNo` and `clsNo` come from the target lookup and form the objecter entry `1!<studentKey>!<gradeNo>!<clsNo>`. */
export interface ExamTarget {studentName: string; studentKey: string; gradeNo: string; clsNo: string}
export interface ExamPeriod {start: string; end: string; testingMinutes: number}
/** The whole change: stage the items, save the paper, assign it. */
export interface ExamPaperChange extends ExamPeriod {
  steps: 'ABC'; family: ExamFamily; testName: string; testgubunNo: number; items: ExamItem[]; sourcePaperNo?: string; courseNo?: number; targets: ExamTarget[];
}
/** The assignment alone, on a paper that is already saved (R13, item 6 of the page): never creates a paper. */
export interface ExamAssignChange extends ExamPeriod {steps: 'C'; family: ExamFamily; testName: string; testno: string; munhangCount: number; targets: ExamTarget[]}
export type ExamChange = ExamPaperChange | ExamAssignChange;
export type ExamItemSource = {kind: 'items'; items: ExamItem[]} | {kind: 'paper'; sourcePaperNo: string} | {kind: 'course'; courseNo: number};
export interface ExamRequest extends ExamPeriod {
  family: ExamFamily; testName: string; testgubunNo: number; source: ExamItemSource; targets: {studentName: string; studentKey: string}[];
}

const invalid = (): never => { throw new GatedWriteRefusal('tool_input_invalid'); };
const publicScope = (): never => { throw new GatedWriteRefusal('public_scope_forbidden'); };
const classDeferred = (): never => { throw new GatedWriteRefusal('class_target_deferred'); };
const isRecord = (v: unknown): v is Record<string, unknown> => v !== null && typeof v === 'object' && !Array.isArray(v);
const isInt = (v: unknown, min: number, max: number): v is number => typeof v === 'number' && Number.isSafeInteger(v) && v >= min && v <= max;
const codePoints = (s: string): number => Array.from(s).length;
// eslint-disable-next-line no-control-regex
const CONTROL = /[\u0000-\u001f\u007f]/;
const text = (v: unknown, max: number): v is string => typeof v === 'string' && v.length > 0 && v === v.trim() && codePoints(v) <= max && !CONTROL.test(v);

/** A member name that is not one of `known`: a class target is deferred, a public or grade-wide scope is forbidden, any other member is invalid. */
function refuseUnknownMembers(raw: Record<string, unknown>, known: readonly string[]): void {
  const unknown = Object.keys(raw).filter(m => !known.includes(m));
  if (unknown.some(m => CLASS_MEMBER.test(m))) classDeferred();
  if (unknown.some(m => PUBLIC_MEMBER.test(m))) publicScope();
  if (unknown.length) invalid();
}
/** `private_chk` is always 1: the paper is never public. Any other value is refused before any request. */
function checkPrivate(raw: Record<string, unknown>): void {
  if (raw.private_chk !== undefined && raw.private_chk !== 1 && raw.private_chk !== '1') publicScope();
}

/** A date-time `YYYY-MM-DDTHH:MM` of the local clock of the page, as its parts and a comparable number. */
export function parseLocalDateTime(value: unknown): {year: number; month: number; day: number; hour: number; minute: number; epoch: number} {
  const m = typeof value === 'string' ? /^(20\d{2})-(\d{2})-(\d{2})T(\d{2}):(\d{2})$/.exec(value) : null;
  if (!m) return invalid();
  const [year, month, day, hour, minute] = m.slice(1).map(Number) as [number, number, number, number, number];
  const epoch = Date.UTC(year, month - 1, day, hour, minute);
  const back = new Date(epoch);
  if (back.getUTCFullYear() !== year || back.getUTCMonth() !== month - 1 || back.getUTCDate() !== day || back.getUTCHours() !== hour || back.getUTCMinutes() !== minute) return invalid();
  return {year, month, day, hour, minute, epoch};
}
function qualifyPeriod(r: Record<string, unknown>): ExamPeriod {
  const start = parseLocalDateTime(r.start), end = parseLocalDateTime(r.end);
  if (start.epoch >= end.epoch) return invalid();
  if (!isInt(r.testingMinutes, 1, 300)) return invalid();
  return {start: r.start as string, end: r.end as string, testingMinutes: r.testingMinutes};
}

const RANK: Record<ExamItemType, number> = {obj: 0, sbj: 1, des: 2};
/**
 * A source paper's rows in the OMR order obj, then sbj, then des, keeping the source order inside each type. `changed` is true when the order differed
 * (the preview's `itemOrder` is `grouped_by_type`, else `source`).
 */
export function groupItemsByType<T extends {type: ExamItemType}>(items: readonly T[]): {items: T[]; changed: boolean} {
  const grouped = [...items].sort((a, b) => RANK[a.type] - RANK[b.type]);
  return {items: grouped, changed: grouped.some((item, i) => item !== items[i])};
}
/** 1 to 30 items in the OMR order obj, then sbj, then des; any other order is `invalid_item_order`. */
export function qualifyItems(value: unknown, options: {exactly?: number; unique?: boolean} = {}): ExamItem[] {
  if (!Array.isArray(value) || value.length < 1) return invalid();
  if (value.length > MAX_ITEMS) throw new GatedWriteRefusal('too_many_items');
  const items = value.map(raw => {
    if (!isRecord(raw) || Object.keys(raw).sort().join(',') !== 'key,type') return invalid();
    if (typeof raw.key !== 'string' || !ITEM_KEY.test(raw.key) || Number(raw.key) > MAX_EXAM_KEY) return invalid();
    if (typeof raw.type !== 'string' || !(ITEM_TYPES as readonly string[]).includes(raw.type)) return invalid();
    return {key: raw.key, type: raw.type as ExamItemType};
  });
  for (let i = 1; i < items.length; i++) if (RANK[items[i]!.type] < RANK[items[i - 1]!.type]) throw new GatedWriteRefusal('invalid_item_order');
  if (options.unique && new Set(items.map(i => i.key + ':' + i.type)).size !== items.length) return invalid();
  if (options.exactly !== undefined && items.length !== options.exactly) return invalid();
  return items;
}

/** The target string of the ledger and the objecter: `iden!priNo!gradeNo!classNo`, iden 1 (one student). */
export const targetString = (t: Pick<ExamTarget, 'studentKey' | 'gradeNo' | 'clsNo'>): string => `1!${t.studentKey}!${t.gradeNo}!${t.clsNo}`;
export const sortedTargetStrings = (targets: readonly ExamTarget[]): string[] => targets.map(targetString).sort();
/** `scopeKey` = `<family>:` + SHA-256 of the sorted target strings joined by newline (R11 compares the kept lists, not this hash). */
export const examScopeKey = (family: ExamFamily, targets: readonly ExamTarget[]): string => family + ':' + sha256Hex(sortedTargetStrings(targets).join('\n'));
/** The student keys of target strings (keys only), as a refusal lists them for a review that names other students. */
export const studentKeysOfTargets = (strings: readonly string[]): string[] => strings.map(s => s.split('!')[1]!).sort((a, b) => Number(a) - Number(b));

/**
 * A review request: refuses `class_target_deferred` for a class target (iden 2 or a class member), `public_scope_forbidden` for a grade
 * (iden 3), an academy-wide or public member, or a missing or empty target list, and `tool_input_invalid` for any other input that does
 * not fit. Found from the input alone: 0 HTTP requests.
 */
export function qualifyExamRequest(raw: unknown): ExamRequest {
  if (!isRecord(raw)) return invalid();
  refuseUnknownMembers(raw, REQUEST_MEMBERS);
  checkPrivate(raw);
  if (raw.targets === undefined || !Array.isArray(raw.targets) || raw.targets.length === 0) return publicScope();
  for (const t of raw.targets) {
    if (!isRecord(t)) return invalid();
    refuseUnknownMembers(t, TARGET_MEMBERS);
    if (t.iden !== undefined && t.iden !== 1) { if (t.iden === 2) return classDeferred(); if (t.iden === 3) return publicScope(); return invalid(); }
  }
  if (raw.targets.length > MAX_TARGETS) return invalid();
  const targets = raw.targets.map(t => {
    const r = t as Record<string, unknown>;
    if (!text(r.studentName, MAX_STUDENT_NAME) || typeof r.studentKey !== 'string' || !KEY.test(r.studentKey)) return invalid();
    return {studentName: r.studentName, studentKey: r.studentKey};
  });
  if (new Set(targets.map(t => t.studentKey)).size !== targets.length) return invalid();
  const family = raw.family;
  if (family !== 'fa' && family !== 'da') return invalid();
  if (!text(raw.testName, MAX_TEST_NAME)) return invalid();
  if (!isInt(raw.testgubunNo, 1, 2147483647)) return invalid();
  const period = qualifyPeriod(raw);
  let source: ExamItemSource;
  if (family === 'fa') {
    if (raw.courseNo !== undefined) return invalid();
    if ((raw.sourcePaperNo === undefined) === (raw.items === undefined)) return invalid();
    if (raw.sourcePaperNo !== undefined) {
      if (!paperNoQualified(raw.sourcePaperNo)) return invalid();
      source = {kind: 'paper', sourcePaperNo: raw.sourcePaperNo};
    } else source = {kind: 'items', items: qualifyItems(raw.items, {unique: true})};
  } else {
    if (raw.sourcePaperNo !== undefined || raw.items !== undefined) return invalid();
    if (!isInt(raw.courseNo, 1, 2147483647)) return invalid();
    source = {kind: 'course', courseNo: raw.courseNo};
  }
  return {family, testName: raw.testName as string, testgubunNo: raw.testgubunNo as number, source, targets, ...period};
}

function qualifyChangeTargets(value: unknown): ExamTarget[] {
  if (!Array.isArray(value) || value.length < 1) return publicScope();
  if (value.length > MAX_TARGETS) return invalid();
  const targets = value.map(raw => {
    if (!isRecord(raw)) return invalid();
    if (raw.iden === 2) return classDeferred();
    if (raw.iden === 3) return publicScope();
    refuseUnknownMembers(raw, CHANGE_TARGET_MEMBERS);
    if (Object.keys(raw).length !== CHANGE_TARGET_MEMBERS.length) return invalid();
    if (!text(raw.studentName, MAX_STUDENT_NAME) || typeof raw.studentKey !== 'string' || !KEY.test(raw.studentKey)) return invalid();
    if (typeof raw.gradeNo !== 'string' || !/^[0-9]{1,15}$/.test(raw.gradeNo) || typeof raw.clsNo !== 'string' || !/^[0-9]{1,15}$/.test(raw.clsNo)) return invalid();
    return {studentName: raw.studentName, studentKey: raw.studentKey, gradeNo: raw.gradeNo, clsNo: raw.clsNo};
  });
  const strings = targets.map(targetString);
  if (new Set(strings).size !== strings.length || new Set(targets.map(t => t.studentKey)).size !== targets.length) return invalid();
  if (!strings.every(s => TARGET_STRING.test(s))) return invalid();
  return targets.sort((a, b) => (targetString(a) < targetString(b) ? -1 : 1));
}

/** The reviewed change of a save call, requalified: R7 refusals keep their code and any other difference is `review_token_invalid` (R4). */
export function qualifyExamChange(raw: unknown): ExamChange {
  try {
    if (!isRecord(raw)) return invalid();
    refuseUnknownMembers(raw, CHANGE_MEMBERS);
    checkPrivate(raw);
    if (raw.private_chk !== undefined) return invalid();
    const family = raw.family;
    if (family !== 'fa' && family !== 'da') return invalid();
    if (!text(raw.testName, MAX_TEST_NAME)) return invalid();
    const targets = qualifyChangeTargets(raw.targets), period = qualifyPeriod(raw);
    if (raw.steps === 'C') {
      if (Object.keys(raw).sort().join(',') !== 'family,munhangCount,start,end,steps,targets,testName,testingMinutes,testno'.split(',').sort().join(',')) return invalid();
      if (typeof raw.testno !== 'string' || !KEY.test(raw.testno) || !isInt(raw.munhangCount, 1, 200)) return invalid();
      const rebuilt: ExamAssignChange = {steps: 'C', family, testName: raw.testName, testno: raw.testno, munhangCount: raw.munhangCount, targets, ...period};
      if (canonicalJson(rebuilt) !== canonicalJson(raw)) return invalid();
      return rebuilt;
    }
    if (raw.steps !== 'ABC' || !isInt(raw.testgubunNo, 1, 2147483647)) return invalid();
    const items = qualifyItems(raw.items, family === 'da' ? {exactly: MAX_ITEMS} : {});
    const rebuilt: ExamPaperChange = {steps: 'ABC', family, testName: raw.testName, testgubunNo: raw.testgubunNo, items, targets, ...period};
    if (family === 'fa') {
      if (raw.courseNo !== undefined) return invalid();
      if (raw.sourcePaperNo !== undefined) { if (!paperNoQualified(raw.sourcePaperNo)) return invalid(); rebuilt.sourcePaperNo = raw.sourcePaperNo; }
    } else {
      if (raw.sourcePaperNo !== undefined || !isInt(raw.courseNo, 1, 2147483647)) return invalid();
      rebuilt.courseNo = raw.courseNo;
    }
    if (canonicalJson(rebuilt) !== canonicalJson(raw)) return invalid();
    return rebuilt;
  } catch (error) {
    if (error instanceof GatedWriteRefusal && (error.code === 'public_scope_forbidden' || error.code === 'class_target_deferred')) throw error;
    throw new GatedWriteRefusal('review_token_invalid');
  }
}

// ---- answer parsers -------------------------------------------------------------------------------

function parseJsonRecord(body: string, code: string): Record<string, unknown> {
  let value: unknown;
  try { value = JSON.parse(body); } catch { throw new GatedWriteRefusal(code); }
  if (!isRecord(value)) throw new GatedWriteRefusal(code);
  return value;
}
const digits = (v: unknown, max = 15): string | null => {
  const s = typeof v === 'number' && Number.isSafeInteger(v) && v >= 0 ? String(v) : typeof v === 'string' ? v : null;
  return s !== null && new RegExp(`^[0-9]{1,${max}}$`).test(s) ? s : null;
};

export const studentListForm = (studentName: string): {reqCmd: string; studentName: string} => ({reqCmd: 'getStudentList', studentName});
export const STUDENT_LOOKUP_PATH = OBJECT_SELECT_PATH;
/**
 * The answer of `getStudentList` for one target: `{result: 'OK', student_list: [...]}` (any other shape is `target_lookup_unreadable`). Exactly one
 * row holds `String(pri_no) === studentKey` and `student_name === studentName`, else `target_not_found`. Gives the frozen target and its `cls_name`
 * (the review's preview only; no other student data is kept).
 */
export function parseStudentList(body: string, want: {studentName: string; studentKey: string}): {target: ExamTarget; clsName: string} {
  const data = parseJsonRecord(body, 'target_lookup_unreadable');
  if (data.result !== 'OK' || !Array.isArray(data.student_list) || data.student_list.length > 500) throw new GatedWriteRefusal('target_lookup_unreadable');
  const rows = data.student_list.map(row => { if (!isRecord(row)) throw new GatedWriteRefusal('target_lookup_unreadable'); return row; });
  const match = rows.filter(r => digits(r.pri_no) === want.studentKey && r.student_name === want.studentName);
  if (match.length !== 1) throw new GatedWriteRefusal('target_not_found');
  const row = match[0]!, gradeNo = digits(row.grade_no), clsNo = digits(row.cls_no);
  if (gradeNo === null || clsNo === null || typeof row.cls_name !== 'string') throw new GatedWriteRefusal('target_lookup_unreadable');
  return {target: {studentName: want.studentName, studentKey: want.studentKey, gradeNo, clsNo}, clsName: row.cls_name.slice(0, 60)};
}

export const coursePaperForm = (courseNo: number, testgubunNo: number): Record<string, string> =>
  ({reqCmd: 'GetPaperNo', assNo: '1000', courseNo: String(courseNo), level_type: testgubunNo === 1320 ? '1' : '0'});
export const COURSE_PAPER_PATH = DIAGNOST_MANAGE_PATH;
/** The paper number of a DA course: `pNo` missing, 0 or not digits is `course_paper_missing`. */
export function parseCoursePaperNo(body: string): string {
  const data = ((): Record<string, unknown> => { try { return parseJsonRecord(body, 'course_paper_missing'); } catch { throw new GatedWriteRefusal('course_paper_missing'); } })();
  const pNo = digits(data.pNo, 9);
  if (pNo === null || Number(pNo) < 1 || !paperNoQualified(pNo)) throw new GatedWriteRefusal('course_paper_missing');
  return pNo;
}

/** The leaf elements of an XML answer, by tag name: the trimmed text of the first element of each name. A document that cannot be parsed throws. */
export const xmlValues = (body: string): Map<string, string> => xmlLeaves(body);
const xmlOrNull = (body: string): Map<string, string> | null => { try { return xmlValues(body); } catch { return null; } };

export interface StagedPaper {startBunitNo: string; startMunitNo: string; endBunitNo: string; endMunitNo: string; totalCnt: number; totalObject: string; totalSubject: string; totalDescript: string; bookNo: string; subjectNo: string; gradeNo: string}
/** The answer of step A's `SaveMunhang`: `result_ok` with the staged totals, `result_sqlerr` (`stage_conflict`), anything else unreadable. */
export function parseSaveMunhang(body: string): {kind: 'ok'; staged: StagedPaper} | {kind: 'conflict'} | {kind: 'unreadable'} {
  const v = xmlOrNull(body);
  if (!v) return {kind: 'unreadable'};
  const result = v.get('result');
  if (result === 'result_sqlerr') return {kind: 'conflict'};
  if (result !== 'result_ok') return {kind: 'unreadable'};
  const num = (name: string): string | null => digits(v.get(name));
  const f = {start_bunit_no: num('start_bunit_no'), start_munit_no: num('start_munit_no'), end_bunit_no: num('end_bunit_no'), end_munit_no: num('end_munit_no'),
    total_cnt: num('total_cnt'), total_object: num('total_object'), total_subject: num('total_subject'), total_descript: num('total_descript'),
    book_no: num('book_no'), subject_no: num('subject_no'), grade_no: num('grade_no')};
  if (Object.values(f).some(x => x === null)) return {kind: 'unreadable'};
  const totalCnt = Number(f.total_cnt);
  if (!isInt(totalCnt, 1, 200)) return {kind: 'unreadable'};
  return {kind: 'ok', staged: {startBunitNo: f.start_bunit_no!, startMunitNo: f.start_munit_no!, endBunitNo: f.end_bunit_no!, endMunitNo: f.end_munit_no!, totalCnt,
    totalObject: f.total_object!, totalSubject: f.total_subject!, totalDescript: f.total_descript!, bookNo: f.book_no!, subjectNo: f.subject_no!, gradeNo: f.grade_no!}};
}

/** Step B's pre-write check answer: `count` > 0 is ok, `count` in -7..0 is `check_munhang_<-count>`, anything else `check_munhang_unreadable`. */
export function parseCheckMunhang(body: string): {kind: 'ok'} | {kind: 'refused'; code: string} {
  const v = xmlOrNull(body);
  const raw = v?.get('count');
  if (raw === undefined || !/^-?[0-9]{1,6}$/.test(raw)) return {kind: 'refused', code: 'check_munhang_unreadable'};
  const count = Number(raw);
  if (count > 0) return {kind: 'ok'};
  return count >= -7 ? {kind: 'refused', code: `check_munhang_${-count}`} : {kind: 'refused', code: 'check_munhang_unreadable'};
}

/** The logic value of step B's answer page: `UPLOAD_OK`, `UPLOAD_FAIL`, or null when it shows neither (or both). */
export function parseSaveLogic(body: string): 'UPLOAD_OK' | 'UPLOAD_FAIL' | null {
  const ok = /UPLOAD_OK/.test(body), fail = /UPLOAD_FAIL/.test(body);
  return ok === fail ? null : ok ? 'UPLOAD_OK' : 'UPLOAD_FAIL';
}

/**
 * The `Validate` answer of step C: `{kind: 'success'}` (no target holds the paper), `{kind: 'names', text}` (the names of students who already have it, never
 * echoed), or `{kind: 'unreadable'}` when the XML cannot be parsed or holds no `result`.
 */
export function parseValidate(body: string): {kind: 'success'} | {kind: 'names'; text: string} | {kind: 'unreadable'} {
  const v = xmlOrNull(body);
  const result = v?.get('result');
  if (result === undefined) return {kind: 'unreadable'};
  return result === 'success' ? {kind: 'success'} : {kind: 'names', text: result};
}
/** Whether a non-success `Validate` result names every target (step C's readback and the reconciling review). */
export const validateNamesAll = (text: string, targets: readonly ExamTarget[]): boolean => targets.every(t => text.includes(t.studentName));
export const validateNamesSome = (text: string, targets: readonly ExamTarget[]): boolean => targets.some(t => text.includes(t.studentName));

/** The saved paper of `TestPageRegServlet?p_process=AutoModify&p_original_testno=<testno>`: its form1 paper number and name, or null when the page has no form1. */
export function parseAutoModify(html: string): {testno: string; testname: string} | null {
  const {form} = readForm(html, 'form1');
  if (!form || !form.controls.some(c => c.tag === 'input' && c.name === 'p_original_testno')) return null;
  const value = (name: string) => form.controls.find(c => c.tag === 'input' && c.name === name)?.value ?? '';
  return {testno: value('p_original_testno').trim(), testname: value('testname')};
}

// ---- the creation page ----------------------------------------------------------------------------

/**
 * The form1 members of the live Auto page (item 2, step B); a member of this list that the page's form1 lacks is `wire_hash_mismatch`. The page's other form
 * `userForm` (`fran_no`, `mem_type`, `p_groupno`, `p_prino`, `p_url`, `pri_no`) and the input `lecture_key` outside both forms are not members of form1 and are
 * neither required nor posted. The da form1 adds `courseNo` and `useterm`, handled apart.
 */
export const FA_FORM_MEMBERS = ['book', 'bunit1', 'bunit2', 'descriptquestion', 'evalgubun', 'exam_test', 'grade', 'keyword_title', 'makecollege',
  'makedate', 'maker', 'munit1', 'munit2', 'object', 'objectviews', 'p_bookno', 'p_d_cnt', 'p_evalgubunno', 'p_gradeno', 'p_o_cnt', 'p_original_testno',
  'p_prepare_testno', 'p_process', 'p_s_cnt', 'p_subjectno', 'p_testgubunno', 'p_total_cnt', 'paper_type', 'private_chk', 'subject', 'subjectquestion',
  'testgubun', 'testname', 'title_option', 'total'] as const;
export interface CreationPage {
  testno: string;
  /** The form1 members in page order, each at the page's value. */
  members: [string, string][];
  testgubunOptions: string[];
  courseNoOptions: string[] | null;
  makecollege: string | null;
}
/** The `p_original_testno` of an Auto page (the number allocated by the load), or null. */
export function autoPageTestno(html: string): string | null {
  const {form} = readForm(html, 'form1');
  const value = form?.controls.find(c => c.tag === 'input' && c.name === 'p_original_testno')?.value.trim();
  return value !== undefined && KEY.test(value) ? value : null;
}
/**
 * The form1 of the Auto page and its options. Refuses `wire_hash_mismatch` for a page without form1, without the number, or whose form1 has no control
 * named by a member of `FA_FORM_MEMBERS` (a control counts whether or not a browser would submit it), or whose `p_original_testno` or `objectviews` is
 * not submitted; `option_not_on_page` when `testgubunNo` (and for da `courseNo`) is not an option of its select; `make_college_missing` when
 * `makecollege` has no usable option (its selected option, else its first option other than -1 and empty).
 */
export function parseCreationPage(html: string, family: ExamFamily, wanted: {testgubunNo: number; courseNo?: number}): CreationPage {
  const {form, documentSelects} = readForm(html, 'form1');
  if (!form) throw new GatedWriteRefusal('wire_hash_mismatch');
  // Presence counts every form1 control by name (an unchecked checkbox and a select without an option are members of the form); `members` stays what a
  // browser posts, so step B posts neither of those two from the page.
  const names = new Set(form.controls.map(c => c.name));
  if (FA_FORM_MEMBERS.some(n => !names.has(n))) throw new GatedWriteRefusal('wire_hash_mismatch');
  const members = form.controls.flatMap(submittedMembers);
  // The members a later step reads from `members` (the paper number here, `objectviews` in step B's CheckMunhang) must be submitted, not only present.
  if (!members.some(([n]) => n === 'objectviews')) throw new GatedWriteRefusal('wire_hash_mismatch');
  const testno = members.find(([n]) => n === 'p_original_testno')?.[1].trim() ?? '';
  if (!KEY.test(testno)) throw new GatedWriteRefusal('wire_hash_mismatch');
  const optionsOf = (name: string): string[] | null => {
    const select = documentSelects.find(c => c.name === name);
    return select ? select.options.map(o => o.value) : null;
  };
  const testgubunOptions = optionsOf('testgubun');
  if (testgubunOptions === null) throw new GatedWriteRefusal('wire_hash_mismatch');
  if (!testgubunOptions.includes(String(wanted.testgubunNo))) throw new GatedWriteRefusal('option_not_on_page');
  let courseNoOptions: string[] | null = null;
  if (family === 'da') {
    courseNoOptions = optionsOf('courseNo');
    if (courseNoOptions === null) throw new GatedWriteRefusal('wire_hash_mismatch');
    if (!courseNoOptions.includes(String(wanted.courseNo))) throw new GatedWriteRefusal('option_not_on_page');
  }
  const college = form.controls.find(c => c.tag === 'select' && c.name === 'makecollege');
  const usable = (v: string) => v !== '' && v !== '-1';
  const chosen = college ? (college.options.find(o => o.selected && usable(o.value)) ?? college.options.find(o => usable(o.value))) : undefined;
  if (!chosen) throw new GatedWriteRefusal('make_college_missing');
  return {testno, members, testgubunOptions, courseNoOptions, makecollege: chosen.value};
}

// ---- wire forms -----------------------------------------------------------------------------------

const encodeQuery = (pairs: readonly (readonly [string, string])[]): string => pairs.map(([k, v]) => k + '=' + encodeURIComponent(v)).join('&');
/** The allocating GET of step A's pre-write read (classed write). */
export const autoPath = (family: ExamFamily): string => examRegPath(family) + '?' + encodeQuery([['p_process', 'Auto']]);
/** Step A's write: `SaveMunhang`, a mutating GET. */
export function saveMunhangPath(family: ExamFamily, testno: string, testName: string, items: readonly ExamItem[]): string {
  const code = {obj: '1', sbj: '2', des: '3'} as const;
  return TESTPAGE_SELECT_PATH + '?' + encodeQuery([['reqCmd', 'SaveMunhang'], ['p_original_testno', testno], ['exam_list', items.map(i => `${i.key}|_|${code[i.type]}`).join('|,|')],
    ['ass_no', family === 'fa' ? '1001' : '1000'], ['keyword_title', testName]]);
}
/** Step B's pre-write read: `CheckMunhang` (a read). `objectviews` is the page's own form1 value. */
export function checkMunhangPath(family: ExamFamily, testno: string, staged: StagedPaper, objectviews: string, uniq: number): string {
  return examRegPath(family) + '?' + encodeQuery([['reqCmd', 'CheckMunhang'], ['param', testno], ['param1', '0'], ['s_cnt', staged.totalSubject], ['o_cnt', staged.totalObject],
    ['d_cnt', staged.totalDescript], ['total', String(staged.totalCnt)], ['bunit1', staged.startBunitNo], ['munit1', staged.startMunitNo], ['bunit2', staged.endBunitNo],
    ['munit2', staged.endMunitNo], ['o_views', objectviews], ['param2', family === 'fa' ? '0' : '1'], ['uniq', String(uniq)]]);
}
/** Step B's write path: the multipart POST of the paper (da carries `courseNo` in the query as well). */
export const paperSavePath = (family: ExamFamily, courseNo?: number): string =>
  examRegPath(family) + '?' + encodeQuery(family === 'da' ? [['reqCmd', 'Save'], ['courseNo', String(courseNo)]] : [['reqCmd', 'Save']]);
/**
 * The multipart members of step B: every form1 member of the Auto page at the page's value, except the ones this change sets (item 4). `private_chk` is 1.
 * A member set here keeps the position of the page's first member of that name; `courseNo` (da) is appended when the page lacks it.
 */
export function paperSaveMembers(page: CreationPage, change: ExamPaperChange, staged: StagedPaper): [string, string][] {
  const set = new Map<string, string>([
    ['p_process', 'Save'], ['testname', change.testName], ['keyword_title', change.testName], ['title_option', ''], ['testgubun', String(change.testgubunNo)],
    ['private_chk', '1'], ['total', String(staged.totalCnt)], ['object', staged.totalObject], ['subjectquestion', staged.totalSubject], ['descriptquestion', staged.totalDescript],
    ['grade', staged.gradeNo], ['subject', staged.subjectNo], ['book', staged.bookNo], ['bunit1', staged.startBunitNo], ['munit1', staged.startMunitNo],
    ['bunit2', staged.endBunitNo], ['munit2', staged.endMunitNo], ['p_original_testno', page.testno], ['makecollege', page.makecollege!],
  ]);
  if (change.family === 'da') set.set('courseNo', String(change.courseNo));
  const done = new Set<string>(), out: [string, string][] = [];
  for (const [name, value] of page.members) {
    if (!set.has(name)) { out.push([name, value]); continue; }
    if (done.has(name)) continue;
    done.add(name); out.push([name, set.get(name)!]);
  }
  for (const [name, value] of set) if (!done.has(name)) out.push([name, value]);
  return out;
}
/** Readback of the saved paper (a read): `AutoModify` of the paper number. */
export const autoModifyPath = (family: ExamFamily, testno: string): string => examRegPath(family) + '?' + encodeQuery([['p_process', 'AutoModify'], ['p_original_testno', testno]]);
export const assignMainPath = (family: ExamFamily, testno: string): string => examHangPath(family) + '?' + encodeQuery([['p_process', 'Main'], ['p_no', testno]]);
const part = (n: number) => String(n);
/** The members of step C's `Save` query, in the page's order; `Validate` sends exactly these with only `reqCmd` changed. */
export function assignQuery(reqCmd: 'Save' | 'Validate', change: ExamChange, testno: string, munhangCount: number, dummy: number): [string, string][] {
  const s = parseLocalDateTime(change.start), e = parseLocalDateTime(change.end);
  const ymd = (d: {year: number; month: number; day: number}) => `${d.year}-${d.month}-${d.day}`;
  return [['reqCmd', reqCmd], ['objecter', change.targets.map(t => targetString(t) + '-').sort().join('')], ['title', change.testName], ['name', change.testName],
    ['munhangCount', String(munhangCount)], ['sdate', ymd(s)], ['shour', part(s.hour)], ['sminute', part(s.minute)], ['edate', ymd(e)], ['ehour', part(e.hour)], ['eminute', part(e.minute)],
    ['testingTime', String(change.testingMinutes)], ['assYearMonth', `${s.year}-${String(s.month).padStart(2, '0')}`], ['applyType', '1'], ['pno', testno], ['testingno', ''],
    ['examPlayType', '0'], ['dummy', String(dummy)]];
}
export const assignPath = (family: ExamFamily, reqCmd: 'Save' | 'Validate', change: ExamChange, testno: string, munhangCount: number, dummy: number): string =>
  examHangPath(family) + '?' + encodeQuery(assignQuery(reqCmd, change, testno, munhangCount, dummy));

/** The preview of a review: family, name, kind, item count by type, each target's name and class name, and the period. No other student data. */
export function examPreview(change: ExamChange, clsNames: ReadonlyMap<string, string>, items: readonly ExamItem[] | null, itemOrder?: 'source' | 'grouped_by_type') {
  const counts = {obj: 0, sbj: 0, des: 0};
  for (const i of items ?? []) counts[i.type]++;
  return {family: change.family, testName: change.testName, ...(change.steps === 'ABC' ? {testgubunNo: change.testgubunNo, itemCounts: counts, ...(itemOrder ? {itemOrder} : {})} : {testno: change.testno, munhangCount: change.munhangCount}),
    steps: change.steps, targets: change.targets.map(t => ({studentName: t.studentName, clsName: clsNames.get(t.studentKey) ?? null})),
    period: {start: change.start, end: change.end, testingMinutes: change.testingMinutes}};
}
