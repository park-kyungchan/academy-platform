/** Read-only LMS schedule pages (specs/academy-mcp/lms-reads.md): attendance, the study schedule and the
 * course study manager. Each view is one request of one servlet page (a GET, or the study schedule's own
 * UserSearch POST); this module qualifies the caller's input before any request, builds the request, and parses
 * the page fail closed:
 * - R4: a login page, a session-error page or a password input refuses with `authentication_required`;
 * - R5: a missing form, a list with neither table nor its start or no-result image, or a missing container refuses
 *   with `source_drift`, a page of another teacher or
 *   another month with `scope_conflict`;
 * - R8: a page over 2 MiB, or over 1000 rows, refuses with `source_rejected`;
 * - R9: a cell keeps its label only: no URL (so no video URL) and no cookie is ever returned.
 * The page shapes are pinned by the synthetic fixtures of tests/lms-reads.test.ts; the same HTML shapes are the
 * wire contract, never an inference from another page. */
import {isLoginOrSessionPage} from './courseBookReport';

export const ATTENDANCE_PATH = '/servlet/controller.cct.tutor.AttendanceServlet';
export const COURSE_MANAGE_PATH = '/servlet/controller.coursemanage.CourseManageServlet';
export const TEST_PAGE_LIST_PATH = '/servlet/controller.tutor.base.TestPageListServlet';
export const DIAGNOST_MANAGE_PATH = '/servlet/controller.tutor.diag.DiagnostManageServlet';
export const LMS_SCHEDULE_VIEWS = ['attendance', 'study_schedule', 'course_study_manager', 'test_results'] as const;
export type LmsScheduleView = typeof LMS_SCHEDULE_VIEWS[number];
export const MAX_LMS_PAGE_BYTES = 2 * 1024 * 1024;
export const MAX_LMS_ROWS = 1000;
export const MAX_LMS_PERIOD_DAYS = 31;
/** The period of view `test_results` only (the other views keep 31 days). */
export const MAX_LMS_TEST_RESULTS_PERIOD_DAYS = 92;
/** The families of view `test_results`: the student's DA, FA and NA attempts, and the new-student diagnosis results. */
export const TEST_RESULT_FAMILIES = ['da', 'fa', 'na', 'new_diag'] as const;
export type TestResultFamily = typeof TEST_RESULT_FAMILIES[number];
/** The `ass_no` of the search URL and the check box of the form for each student family (live page TestPageListServlet). */
const STUDENT_TEST_FAMILIES = {
  da: {assNo: '1000', flag: 'check_da_test'},
  fa: {assNo: '1001', flag: 'check_fa_test'},
  na: {assNo: '1002', flag: 'check_na_test'},
} as const;
/** The values of the `sortDate` select of DiagnostManageServlet `TestResultList`. */
export const NEW_DIAG_SORT_DATES = ['1', '2', '3', '11', '12', '13', '14', '15', '16'] as const;
const MAX_CELL_CHARS = 500;

export type LmsReadRefusalCode = 'authentication_required' | 'source_rejected' | 'source_drift' | 'scope_conflict';
/** A page refusal: the message is the public code and nothing else (no page text, URL or cookie). */
export class LmsReadRefusal extends Error {
  constructor(readonly code: LmsReadRefusalCode) { super(code); this.name = 'LmsReadRefusal'; }
}
function refuse(code: LmsReadRefusalCode): never { throw new LmsReadRefusal(code); }

type Parse = (html: string, mime: DOMParserSupportedType) => Document;

/** R8: the bytes of the page, as UTF-8. */
export function assertLmsPageSize(html: string): void {
  if (Buffer.byteLength(html, 'utf8') > MAX_LMS_PAGE_BYTES) refuse('source_rejected');
}
/** R4: a login page, a session-error page or a page with a password input. */
export function assertNotLoginPage(html: string): void {
  if (isLmsLoginPage(html)) refuse('authentication_required');
}
/** A login page, a session-error page, a password input, or a script that sends the browser to the login page. */
export function isLmsLoginPage(html: string): boolean {
  return isLoginOrSessionPage(html) || /c_login\.jsp/i.test(html);
}
/** One page body, read with the 2 MiB bound before it is decoded in full (R8). */
export async function readBoundedLmsPage(response: Response): Promise<string> {
  const declared = Number(response.headers.get('content-length') ?? 0);
  if (Number.isFinite(declared) && declared > MAX_LMS_PAGE_BYTES) refuse('source_rejected');
  const reader = response.body?.getReader();
  if (!reader) { const text = await response.text(); assertLmsPageSize(text); return text; }
  const chunks: Uint8Array[] = [];
  let total = 0;
  for (;;) {
    const {done, value} = await reader.read();
    if (done) break;
    total += value.byteLength;
    if (total > MAX_LMS_PAGE_BYTES) { await reader.cancel().catch(() => undefined); refuse('source_rejected'); }
    chunks.push(value);
  }
  return new TextDecoder().decode(Buffer.concat(chunks));
}

// ---- input -------------------------------------------------------------------------------------

export interface LmsScheduleInput {
  view: LmsScheduleView;
  date?: string;
  period?: {from: string; through: string};
  groupKey?: string; studentKey?: string; gradeNo?: string; semesterNo?: string; levelNo?: string; courseKey?: string; status?: string;
  /** View `test_results` only: the family, the student's name (da, fa, na: resolved by the backend; new_diag: a literal filter) and, for new_diag, the page's `sortDate`. */
  family?: TestResultFamily; displayName?: string; sortDate?: string;
}
/** The members each view takes besides `view`; any other member refuses before a request. */
export const LMS_SCHEDULE_ALLOWED: Readonly<Record<LmsScheduleView, readonly string[]>> = {
  attendance: ['date', 'period', 'groupKey', 'studentKey'],
  study_schedule: ['gradeNo', 'groupKey', 'studentKey', 'courseKey'],
  course_study_manager: ['gradeNo', 'semesterNo', 'levelNo', 'status'],
  test_results: ['family', 'period', 'displayName', 'sortDate'],
};
const MEMBERS = ['date', 'period', 'groupKey', 'studentKey', 'gradeNo', 'semesterNo', 'levelNo', 'courseKey', 'status', 'family', 'displayName', 'sortDate'] as const;
export const LMS_DIRECT_KEY = /^[1-9][0-9]{0,15}$/;
export const LMS_STATUS = /^[A-Za-z0-9_가-힣-]{1,32}$/;
const KEY_MEMBERS = ['groupKey', 'studentKey', 'gradeNo', 'semesterNo', 'levelNo', 'courseKey'] as const;

/** A real calendar date written `YYYY-MM-DD` (R7). */
export function isCalendarDate(value: unknown): value is string {
  if (typeof value !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const [y, m, d] = value.split('-').map(Number) as [number, number, number];
  const parsed = new Date(Date.UTC(y, m - 1, d));
  return y >= 1 && parsed.getUTCFullYear() === y && parsed.getUTCMonth() === m - 1 && parsed.getUTCDate() === d;
}
const dayNumber = (value: string) => Date.UTC(Number(value.slice(0, 4)), Number(value.slice(5, 7)) - 1, Number(value.slice(8, 10))) / 86_400_000;

export interface LmsInputIssue {path: (string | number)[]; message: string}
/** Every reason the input cannot be sent; empty when it can. Used by the tool's schema and again by the client. */
export function lmsScheduleInputIssues(input: Record<string, unknown>): LmsInputIssue[] {
  const issues: LmsInputIssue[] = [];
  const view = input.view as LmsScheduleView;
  if (!(LMS_SCHEDULE_VIEWS as readonly string[]).includes(view)) return [{path: ['view'], message: 'view_unsupported'}];
  const allowed = LMS_SCHEDULE_ALLOWED[view];
  for (const member of MEMBERS) {
    if (input[member] === undefined) continue;
    if (!allowed.includes(member)) issues.push({path: [member], message: 'member_not_for_view'});
  }
  for (const member of KEY_MEMBERS) {
    const value = input[member];
    if (value !== undefined && allowed.includes(member) && !(typeof value === 'string' && LMS_DIRECT_KEY.test(value))) issues.push({path: [member], message: 'direct_key_invalid'});
  }
  if (input.status !== undefined && allowed.includes('status') && !(typeof input.status === 'string' && LMS_STATUS.test(input.status))) issues.push({path: ['status'], message: 'status_invalid'});
  if (input.displayName !== undefined && allowed.includes('displayName')
    && !(typeof input.displayName === 'string' && input.displayName.trim() !== '' && input.displayName.length <= 200 && !/[\u0000-\u001f\u007f]/.test(input.displayName))) issues.push({path: ['displayName'], message: 'display_name_invalid'});
  if (view === 'test_results') { issues.push(...testResultsIssues(input)); return issues; }
  if (view !== 'attendance') return issues;
  if (input.date !== undefined && input.period !== undefined) issues.push({path: ['period'], message: 'date_or_period_not_both'});
  if (input.date !== undefined && !isCalendarDate(input.date)) issues.push({path: ['date'], message: 'date_not_a_calendar_date'});
  // One GET of AttendanceServlet returns one month; a longer span would silently omit rows.
  if (input.period !== undefined) issues.push(...periodIssues(input.period, MAX_LMS_PERIOD_DAYS, 'period_over_31_days', true));
  return issues;
}
/** The issues of one `period` {from, through}: both members real dates, from not after through, at most `maxDays` days, and within one month when asked. */
function periodIssues(period: unknown, maxDays: number, overMessage: string, oneMonth: boolean): LmsInputIssue[] {
  const p = period as {from?: unknown; through?: unknown} | null;
  const keys = p && typeof p === 'object' ? Object.keys(p).sort().join(',') : '';
  if (keys !== 'from,through') return [{path: ['period'], message: 'period_from_through_required'}];
  if (!isCalendarDate(p!.from) || !isCalendarDate(p!.through)) {
    const out: LmsInputIssue[] = [];
    if (!isCalendarDate(p!.from)) out.push({path: ['period', 'from'], message: 'date_not_a_calendar_date'});
    if (!isCalendarDate(p!.through)) out.push({path: ['period', 'through'], message: 'date_not_a_calendar_date'});
    return out;
  }
  const span = dayNumber(p!.through as string) - dayNumber(p!.from as string) + 1;
  if (span < 1) return [{path: ['period'], message: 'period_from_after_through'}];
  if (span > maxDays) return [{path: ['period'], message: overMessage}];
  if (oneMonth && (p!.from as string).slice(0, 7) !== (p!.through as string).slice(0, 7)) return [{path: ['period'], message: 'attendance_period_crosses_month'}];
  return [];
}
/** View `test_results`: the family is required; da, fa and na need the student's name and a period of at most 92 days (the search is by period) and take no
 * `sortDate`; new_diag takes no period, and `sortDate` (one of the page's values) or none. */
function testResultsIssues(input: Record<string, unknown>): LmsInputIssue[] {
  const family = input.family;
  if (family === undefined) return [{path: ['family'], message: 'family_required'}];
  if (!(TEST_RESULT_FAMILIES as readonly unknown[]).includes(family)) return [{path: ['family'], message: 'family_invalid'}];
  const issues: LmsInputIssue[] = [];
  if (family === 'new_diag') {
    if (input.period !== undefined) issues.push({path: ['period'], message: 'period_not_for_new_diag'});
    if (input.sortDate !== undefined && !(NEW_DIAG_SORT_DATES as readonly unknown[]).includes(input.sortDate)) issues.push({path: ['sortDate'], message: 'sort_date_invalid'});
    return issues;
  }
  if (input.displayName === undefined) issues.push({path: ['displayName'], message: 'display_name_required'});
  if (input.sortDate !== undefined) issues.push({path: ['sortDate'], message: 'sort_date_new_diag_only'});
  if (input.period === undefined) issues.push({path: ['period'], message: 'period_required'});
  else issues.push(...periodIssues(input.period, MAX_LMS_TEST_RESULTS_PERIOD_DAYS, 'period_over_92_days', false));
  return issues;
}
export function qualifyLmsScheduleInput(input: unknown): LmsScheduleInput {
  const raw = input && typeof input === 'object' ? input as Record<string, unknown> : {};
  const unknown = Object.keys(raw).filter(k => k !== 'view' && !(MEMBERS as readonly string[]).includes(k));
  if (unknown.length || lmsScheduleInputIssues(raw).length) throw Error('tool_input_invalid');
  return raw as unknown as LmsScheduleInput;
}
/** The month the caller asked for (`YYYY-MM`) of an attendance read, or null when no date or period was given. */
export function requestedMonth(input: LmsScheduleInput): string | null {
  const day = input.date ?? input.period?.from;
  return day ? day.slice(0, 7) : null;
}

/** The four filters of the study schedule search (the page's own onSearch): all four are required. */
export const hasAllStudyScheduleFilters = (input: LmsScheduleInput): boolean =>
  input.view === 'study_schedule' && [input.gradeNo, input.groupKey, input.studentKey, input.courseKey].every(v => v !== undefined);

export interface LmsScheduleRequest {path: string; method: 'GET' | 'POST'; body?: string}
/** The request of one view, in the page's own parameter names (specs/academy-mcp/lms-reads.md "Routes"):
 * - attendance: GET AttendanceServlet Main with the month and filters;
 * - study_schedule with all four filters: the POST of the page's search, `reqCmd=UserSearch` with `grade_no`,
 *   `cls_no`, `student_no` and `course` in the form body; with any of them missing: the start page GET with the
 *   given filters in its query;
 * - course_study_manager: always the `search=1` GET; a filter not given is the page's own "all" value;
 * - test_results with family da, fa or na: the POST of the page's own search (`TestPageListServlet`
 *   `UserBySearchTestResult&ass_no=1000|1001|1002`) with the owner's college key (`context.collegeKey`), the student's name, the period and page 1;
 *   with family new_diag: the GET of `DiagnostManageServlet` `TestResultList`, with `sortDate` when given. */
export function lmsScheduleRequest(input: LmsScheduleInput, context: {collegeKey?: string} = {}): LmsScheduleRequest {
  const q = new URLSearchParams();
  if (input.view === 'test_results') {
    if (input.family === 'new_diag') {
      q.set('reqCmd', 'TestResultList');
      if (input.sortDate !== undefined) q.set('sortDate', input.sortDate);
      return {method: 'GET', path: DIAGNOST_MANAGE_PATH + '?' + q};
    }
    const spec = input.family !== undefined ? STUDENT_TEST_FAMILIES[input.family] : undefined;
    if (!spec || input.displayName === undefined || !input.period || !context.collegeKey) throw Error('tool_input_invalid');
    const form = new URLSearchParams([['clg_no', context.collegeKey], ['cls_no', '0'], ['p_pageno', '1'], [spec.flag, spec.assNo], ['stu_name', input.displayName],
      ['chioce_cls', '0'], ['sort_date1', input.period.from], ['sort_date2', input.period.through]]);
    return {method: 'POST', path: `${TEST_PAGE_LIST_PATH}?p_process=UserBySearchTestResult&ass_no=${spec.assNo}`, body: form.toString()};
  }
  const add = (name: string, value: string | undefined) => { if (value !== undefined) q.set(name, value); };
  if (input.view === 'attendance') {
    q.set('p_process', 'Main');
    const month = requestedMonth(input);
    if (month) { q.set('ym', month.replace('-', '')); q.set('sdate', month); }
    add('grp_seq', input.groupKey); add('stu_obj', input.studentKey);
    return {method: 'GET', path: ATTENDANCE_PATH + '?' + q};
  }
  if (input.view === 'study_schedule') {
    if (hasAllStudyScheduleFilters(input)) {
      const form = new URLSearchParams({reqCmd: 'UserSearch', grade_no: input.gradeNo!, cls_no: input.groupKey!, student_no: input.studentKey!, course: input.courseKey!});
      return {method: 'POST', path: COURSE_MANAGE_PATH, body: form.toString()};
    }
    q.set('reqCmd', 'StudySchedule');
    add('grade_no', input.gradeNo); add('cls_no', input.groupKey); add('student_no', input.studentKey); add('course', input.courseKey);
    return {method: 'GET', path: COURSE_MANAGE_PATH + '?' + q};
  }
  q.set('reqCmd', 'CourseStudyManager');
  q.set('grade_no', input.gradeNo ?? '0'); q.set('semester_no', input.semesterNo ?? '0'); q.set('level_no', input.levelNo ?? '0'); q.set('status', input.status ?? '-1');
  q.set('search', '1');
  return {method: 'GET', path: COURSE_MANAGE_PATH + '?' + q};
}
/** The path (with its query, for a GET) of the request of one view. */
export const lmsScheduleRequestPath = (input: LmsScheduleInput): string => lmsScheduleRequest(input).path;

// ---- page parsing ------------------------------------------------------------------------------

/** Every absolute URL, and every protocol-relative URL that starts a word, replaced by a space. */
export function stripUrls(value: string): string {
  return value.replace(/\bhttps?:\/\/[^\s"'<>]+/gi, ' ').replace(/(^|\s)\/\/[^\s"'<>]+/g, '$1 ');
}
/** A cell label: whitespace collapsed, every URL removed (R9: a video URL embeds a student key), bounded. */
export function lmsCellText(value: string | null | undefined): string {
  return stripUrls(value ?? '').replace(/\s+/g, ' ').trim().slice(0, MAX_CELL_CHARS);
}
export interface LmsOption {key: string; label: string; selected: boolean}

function optionsOf(select: Element): LmsOption[] {
  const out: LmsOption[] = [];
  for (const option of select.querySelectorAll('option')) {
    const key = lmsCellText(option.getAttribute('value'));
    if (!key) continue;
    out.push({key, label: lmsCellText(option.textContent), selected: option.hasAttribute('selected')});
  }
  return out;
}
/** The value of a named control of a form: an input's value, or a select's selected (else first) option. */
function controlValue(form: Element, name: string): string | null {
  const node = form.querySelector(`[name="${name}"]`);
  if (!node) return null;
  if (node.tagName === 'SELECT') {
    const options = [...node.querySelectorAll('option')];
    const picked = options.find(o => o.hasAttribute('selected')) ?? options[0];
    return picked?.getAttribute('value') ?? '';
  }
  return (node as HTMLInputElement).getAttribute('value') ?? '';
}
const hasClasses = (el: Element, ...names: string[]) => { const set = new Set((el.getAttribute('class') ?? '').split(/\s+/)); return names.every(n => set.has(n)); };

function checkTeacher(doc: Document, teacherKey: string, sources: readonly {form: string; field: string}[]): void {
  let seen = 0;
  for (const {form, field} of sources) {
    const node = doc.querySelector(`${form} input[name="${field}"]`);
    if (!node) continue;
    seen++;
    if ((node.getAttribute('value') ?? '') !== teacherKey) refuse('scope_conflict');
  }
  if (seen === 0) refuse('source_drift');
}

function cellsOf(row: Element): string[] {
  return [...row.querySelectorAll(':scope > td')].map(td => lmsCellText(td.textContent));
}
const isEmptyStateRow = (row: Element) => {
  const cells = [...row.querySelectorAll(':scope > td')];
  return cells.length === 1 && Number(cells[0]!.getAttribute('colspan') ?? 1) > 1;
};
function directRows(table: Element): Element[] {
  return [...table.querySelectorAll(':scope > tbody > tr, :scope > thead > tr, :scope > tr')];
}

// attendance

export const ATTENDANCE_HEADERS = ['수업일', '과정명', '수업형태', '출결사항', '메모'] as const;
export interface AttendanceRow {lessonDate: string; courseName: string; lessonType: string; attendanceStatus: string; memo: string}
export interface AttendanceRead {view: 'attendance'; month: string | null; rows: AttendanceRow[]; groups: LmsOption[]; students: LmsOption[]; pageRowCount: number}

const monthOf = (value: string | null, shape: 'ym' | 'sdate'): string | null => {
  if (value === null || value === '') return null;
  const m = (shape === 'ym' ? /^(\d{4})(\d{2})$/ : /^(\d{4})-(\d{2})(?:-\d{2})?$/).exec(value.trim());
  return m ? `${m[1]}-${m[2]}` : 'unparsed';
};
/** The ISO date of a lesson-date literal such as `2040-01-15` or `2040.01.15(월)`; null when it holds none. */
export function lessonDateOf(literal: string): string | null {
  const m = /(\d{4})[-./](\d{1,2})[-./](\d{1,2})/.exec(literal);
  if (!m) return null;
  const iso = `${m[1]}-${m[2]!.padStart(2, '0')}-${m[3]!.padStart(2, '0')}`;
  return isCalendarDate(iso) ? iso : null;
}

export function parseAttendance(html: string, parse: Parse, teacherKey: string, requested: {month: string | null; range?: {from: string; through: string}}): AttendanceRead {
  assertLmsPageSize(html);
  assertNotLoginPage(html);
  const doc = parse(html, 'text/html');
  const form = doc.querySelector('form#form1');
  if (!form) refuse('source_drift');
  const table = [...doc.querySelectorAll('table')].find(t => hasClasses(t, 'content-table', 'thin'));
  if (!table) refuse('source_drift');
  const rows = directRows(table!);
  const headerRow = rows.find(r => r.querySelector(':scope > th'));
  const headers = headerRow ? [...headerRow.querySelectorAll(':scope > th')].map(th => lmsCellText(th.textContent)) : [];
  if (headers.length !== ATTENDANCE_HEADERS.length || headers.some((h, i) => h !== ATTENDANCE_HEADERS[i])) refuse('source_drift');
  checkTeacher(doc, teacherKey, [{form: 'form#form1', field: 'tutor_pri_no'}, {form: 'form[name="userForm"]', field: 'pri_no'}]);
  // The page's own month fields must be the month asked for; none to compare is drift, not a pass.
  const fields = [monthOf(controlValue(form!, 'ym'), 'ym'), monthOf(controlValue(form!, 'sdate'), 'sdate')].filter((v): v is string => v !== null);
  if (requested.month) {
    if (fields.length === 0) refuse('source_drift');
    if (fields.some(v => v !== requested.month)) refuse('scope_conflict');
  }
  const page = fields[0] ?? null;
  const parsed: AttendanceRow[] = [];
  for (const row of rows) {
    if (row === headerRow || !row.querySelector(':scope > td')) continue;
    if (isEmptyStateRow(row)) continue;
    const cells = cellsOf(row);
    if (cells.length !== ATTENDANCE_HEADERS.length) refuse('source_drift');
    parsed.push({lessonDate: cells[0]!, courseName: cells[1]!, lessonType: cells[2]!, attendanceStatus: cells[3]!, memo: cells[4]!});
    if (parsed.length > MAX_LMS_ROWS) refuse('source_rejected');
  }
  let kept = parsed;
  if (requested.range) {
    kept = parsed.filter(r => {
      const iso = lessonDateOf(r.lessonDate);
      if (iso === null) refuse('source_drift');
      return iso >= requested.range!.from && iso <= requested.range!.through;
    });
  }
  const select = (name: string) => { const node = form!.querySelector(`select[name="${name}"]`); return node ? optionsOf(node) : []; };
  return {view: 'attendance', month: page, rows: kept.map(r => ({...r, lessonDate: lessonDateOf(r.lessonDate) ?? r.lessonDate})), groups: select('grp_seq'), students: select('stu_obj'), pageRowCount: parsed.length};
}

// study schedule and course study manager

export interface LmsTableRow {[column: string]: string}
/** `status` is only set when the study schedule was read without its four filters: the page is then the start page. */
export interface StudyScheduleRead {view: 'study_schedule'; status?: 'filters_required'; columns: string[]; rows: LmsTableRow[]; grades: LmsOption[]; classes: LmsOption[]; students: LmsOption[]; courses: LmsOption[]}
export interface CourseStudyManagerRead {view: 'course_study_manager'; columns: string[]; rows: LmsTableRow[]; grades: LmsOption[]; semesters: LmsOption[]; levels: LmsOption[]; statuses: LmsOption[]}

const SCHEDULE_CONTROLS = {
  study_schedule: {selects: ['grade_no', 'cls_no', 'student_no', 'course']},
  course_study_manager: {selects: ['grade_no', 'semester_no', 'level_no', 'status']},
} as const;
/** The image that the page's own list shows instead of a table: the start page (`firstpage.png`) and a search without a result (`no_result.png`). */
const EMPTY_LIST_IMAGE = /(?:^|\/)(?:firstpage|no_result)\.png(?:[?#].*)?$/i;
const MAX_SPAN = 64;
const spanOf = (cell: Element, name: 'colspan' | 'rowspan', max: number) => {
  const n = Number.parseInt(cell.getAttribute(name) ?? '1', 10);
  return Number.isFinite(n) && n > 1 ? Math.min(n, max) : 1;
};

/** The columns of a header row, a header cell of colspan n counted n times. */
function headerColumns(headerRow: Element | undefined): string[] {
  const out: string[] = [];
  for (const th of headerRow ? headerRow.querySelectorAll(':scope > th') : []) {
    const text = lmsCellText(th.textContent);
    for (let k = spanOf(th, 'colspan', MAX_SPAN); k > 0; k--) out.push(text);
  }
  return out.map((h, i) => h || `column${i + 1}`);
}
/** The data rows of a table as grids of cell labels: a cell of colspan n fills n columns, a cell of rowspan n fills its column of the next n-1 rows too. */
function expandedRows(table: Element, headerRow: Element | undefined): string[][] {
  const carry = new Map<number, {text: string; left: number}>();
  const out: string[][] = [];
  for (const row of directRows(table)) {
    if (row === headerRow || !row.querySelector(':scope > td')) continue;
    if (carry.size === 0 && isEmptyStateRow(row)) continue;
    const cells: string[] = [];
    let col = 0;
    const takeCarried = () => {
      for (let held = carry.get(col); held; held = carry.get(col)) {
        cells[col] = held.text;
        if (--held.left <= 0) carry.delete(col);
        col++;
      }
    };
    for (const td of row.querySelectorAll(':scope > td')) {
      takeCarried();
      const text = lmsCellText(td.textContent), rows = spanOf(td, 'rowspan', MAX_LMS_ROWS + 1);
      for (let k = spanOf(td, 'colspan', MAX_SPAN); k > 0; k--, col++) {
        cells[col] = text;
        if (rows > 1) carry.set(col, {text, left: rows - 1});
      }
    }
    takeCarried();
    out.push(Array.from(cells, c => c ?? ''));
    if (out.length > MAX_LMS_ROWS) refuse('source_rejected');
  }
  return out;
}

function parseContentList(view: 'study_schedule' | 'course_study_manager', html: string, parse: Parse, teacherKey: string) {
  assertLmsPageSize(html);
  assertNotLoginPage(html);
  const doc = parse(html, 'text/html');
  const spec = SCHEDULE_CONTROLS[view];
  const form = doc.querySelector('form#form1');
  if (!form) refuse('source_drift');
  // The start page holds `reqCmd` without a value: the input must exist, its value is not compared.
  if (!form!.querySelector('input[name="reqCmd"]')) refuse('source_drift');
  const selects = new Map<string, Element>();
  for (const name of spec.selects) {
    const node = form!.querySelector(`select[name="${name}"]`);
    if (!node) refuse('source_drift');
    selects.set(name, node!);
  }
  const list = [...doc.querySelectorAll('div')].find(d => hasClasses(d, 'content-list'));
  if (!list) refuse('source_drift');
  if (!doc.querySelector('form[name="userForm"] input[name="pri_no"]')) refuse('source_drift');
  checkTeacher(doc, teacherKey, [{form: 'form[name="userForm"]', field: 'pri_no'}]);
  const tables = [...list!.querySelectorAll('table')].filter(t => hasClasses(t, 'content-table'));
  const imageOnly = [...list!.querySelectorAll('img')].some(img => EMPTY_LIST_IMAGE.test(img.getAttribute('src') ?? ''));
  // A list is a table, or the image of the start page or of a search without a result; anything else is drift.
  if (tables.length === 0 && !imageOnly) refuse('source_drift');
  const columns: string[] = [];
  const rows: LmsTableRow[] = [];
  for (const table of tables) {
    const headerRow = directRows(table).find(r => r.querySelector(':scope > th'));
    const headers = headerColumns(headerRow);
    const unique = headers.map((h, i) => headers.indexOf(h) === i ? h : `${h}#${i + 1}`);
    if (columns.length === 0) columns.push(...unique);
    for (const cells of expandedRows(table, headerRow)) {
      const out: LmsTableRow = {};
      for (let i = 0; i < Math.max(cells.length, unique.length); i++) out[unique[i] ?? `column${i + 1}`] = cells[i] ?? '';
      rows.push(out);
      if (rows.length > MAX_LMS_ROWS) refuse('source_rejected');
    }
  }
  const options = (name: string) => optionsOf(selects.get(name)!);
  return {columns, rows, options};
}

export function parseStudySchedule(html: string, parse: Parse, teacherKey: string, filtersRequired = false): StudyScheduleRead {
  const {columns, rows, options} = parseContentList('study_schedule', html, parse, teacherKey);
  const lists = {grades: options('grade_no'), classes: options('cls_no'), students: options('student_no'), courses: options('course')};
  // Without its four filters the page is the start page: no rows, the options only.
  return filtersRequired ? {view: 'study_schedule', status: 'filters_required', columns: [], rows: [], ...lists} : {view: 'study_schedule', columns, rows, ...lists};
}
export function parseCourseStudyManager(html: string, parse: Parse, teacherKey: string): CourseStudyManagerRead {
  const {columns, rows, options} = parseContentList('course_study_manager', html, parse, teacherKey);
  return {view: 'course_study_manager', columns, rows, grades: options('grade_no'), semesters: options('semester_no'), levels: options('level_no'), statuses: options('status')};
}

// test results: the inline `testResultList` of TestPageListServlet and DiagnostManageServlet

/** One attempt of a student's DA, FA or NA test (row of `TestPageListServlet` `UserBySearchTestResult`). `classNo` is 0 on every live row. */
export interface TestAttemptRow {paperNo: string; testingNo: string; studentKey: string; classNo: string; title: string; applyDate: string; score: number | null}
/** One new-student diagnosis result (row of `DiagnostManageServlet` `TestResultList`); `applyCode` and `paperNo` are the keys of `new_diag`. */
export interface DiagnosisResultRow {paperNo: string; applyCode: string; studentName: string; applyDate: string; title: string; score: number | null}
export interface TestResultsRead {
  view: 'test_results'; family: TestResultFamily; page: 1; pageRowCount: number;
  student?: {studentId: string; displayName: string}; period?: {from: string; through: string}; sortDate?: string;
  /** Rows of another student on the page (the page's name search also matches longer names): left out and counted, never returned. */
  otherStudentRowsOmitted?: number;
  rows: (TestAttemptRow | DiagnosisResultRow)[];
}

/** The text between the quotes of a JavaScript single-quoted string literal, decoded the way the language reads it (`\'`, `\"`, `\\`, `\n`, `\uXXXX` and
 * the other escapes turned back into characters). Nothing is evaluated; an escape this does not know (an octal one, a short `\x` or `\u`) refuses with `source_drift`. */
export function decodeJsSingleQuotedString(body: string): string {
  let out = '';
  const hex = (from: number, length: number): number => {
    const digits = body.slice(from, from + length);
    if (digits.length !== length || !/^[0-9a-fA-F]+$/.test(digits)) refuse('source_drift');
    return Number.parseInt(digits, 16);
  };
  for (let i = 0; i < body.length; i++) {
    const c = body[i]!;
    if (c !== '\\') { out += c; continue; }
    const next = body[++i];
    if (next === undefined) refuse('source_drift');
    switch (next) {
      case 'n': out += '\n'; break;
      case 'r': out += '\r'; break;
      case 't': out += '\t'; break;
      case 'b': out += '\b'; break;
      case 'f': out += '\f'; break;
      case 'v': out += '\v'; break;
      case '0':
        if (/[0-9]/.test(body[i + 1] ?? '')) refuse('source_drift');
        out += '\0'; break;
      case 'x': out += String.fromCharCode(hex(i + 1, 2)); i += 2; break;
      case 'u':
        if (body[i + 1] === '{') {
          const end = body.indexOf('}', i + 2);
          if (end < 0 || end - (i + 2) < 1 || end - (i + 2) > 6) refuse('source_drift');
          const point = hex(i + 2, end - (i + 2));
          if (point > 0x10ffff) refuse('source_drift');
          out += String.fromCodePoint(point); i = end;
        } else { out += String.fromCharCode(hex(i + 1, 4)); i += 4; }
        break;
      case '\r': if (body[i + 1] === '\n') i++; break; // a line continuation
      case '\n': case '\u2028': case '\u2029': break;
      default:
        if (/[1-9]/.test(next)) refuse('source_drift');
        out += next; // \' \" \\ \/ and any other escaped character stands for itself
    }
  }
  return out;
}
/** The rows of `EXTERN_DIALOG = { testResultList: JSON.parse('...') }`: the literal is decoded (never evaluated), then parsed as JSON. A missing or malformed
 * literal, JSON that is not an array of objects, refuses with `source_drift`; more than 1000 rows with `source_rejected`. */
export function readInlineTestResultList(html: string): Record<string, unknown>[] {
  const dialog = /\bEXTERN_DIALOG\s*=\s*\{/.exec(html);
  if (!dialog) refuse('source_drift');
  const rest = html.slice(dialog!.index + dialog![0].length);
  const start = /\btestResultList\s*:\s*JSON\.parse\(\s*'/.exec(rest);
  if (!start) refuse('source_drift');
  const from = start!.index + start![0].length;
  let end = from;
  for (; end < rest.length; end++) {
    const c = rest[end]!;
    if (c === '\\') { end++; continue; }
    if (c === "'") break;
    if (c === '\n' || c === '\r') refuse('source_drift');
  }
  if (end >= rest.length || !/^\s*\)/.test(rest.slice(end + 1, end + 40))) refuse('source_drift');
  let parsed: unknown;
  try { parsed = JSON.parse(decodeJsSingleQuotedString(rest.slice(from, end))); } catch (error) {
    if (error instanceof LmsReadRefusal) throw error;
    refuse('source_drift');
  }
  if (!Array.isArray(parsed)) refuse('source_drift');
  const list = parsed as unknown[];
  if (list.length > MAX_LMS_ROWS) refuse('source_rejected');
  return list.map(row => {
    if (row === null || typeof row !== 'object' || Array.isArray(row)) refuse('source_drift');
    return row as Record<string, unknown>;
  });
}
const rowKey = (value: unknown, zeroAllowed = false): string => {
  const text = typeof value === 'string' || typeof value === 'number' ? String(value) : '';
  if (!(zeroAllowed ? /^(?:0|[1-9][0-9]{0,15})$/ : LMS_DIRECT_KEY).test(text) || Number(text) > Number.MAX_SAFE_INTEGER) refuse('source_drift');
  return text;
};
const rowText = (value: unknown): string => lmsCellText(typeof value === 'string' || typeof value === 'number' ? String(value) : '');
const rowScore = (value: unknown): number | null => {
  const n = typeof value === 'number' ? value : typeof value === 'string' && /^-?\d+(?:\.\d+)?$/.test(value.trim()) ? Number(value) : NaN;
  return Number.isFinite(n) ? n : null;
};
const isApplyCode = (value: unknown): value is string => typeof value === 'string' && value.length >= 1 && value.length <= 64 && value.trim() === value && !/[\u0000-\u001f\u007f]/.test(value);

/** The test results page of the family, parsed fail closed: the teacher key of its `userForm` (R5), the inline list, and the rows of the resolved student
 * (da, fa, na) or of the literal `displayName` (new_diag). The page holds the first page of the search only. */
export function parseTestResults(input: LmsScheduleInput, html: string, parse: Parse, teacherKey: string, student?: {studentId: string; displayName: string}): TestResultsRead {
  assertLmsPageSize(html);
  assertNotLoginPage(html);
  const doc = parse(html, 'text/html');
  checkTeacher(doc, teacherKey, [{form: 'form[name="userForm"]', field: 'pri_no'}]);
  const list = readInlineTestResultList(html);
  const family = input.family!;
  if (family === 'new_diag') {
    const kept = input.displayName === undefined ? list : list.filter(row => row.studentName === input.displayName);
    const rows: DiagnosisResultRow[] = kept.map(row => {
      if (!isApplyCode(row.applyCode)) refuse('source_drift');
      return {paperNo: rowKey(row.pNo), applyCode: row.applyCode as string, studentName: rowText(row.studentName), applyDate: rowText(row.applyDate), title: rowText(row.testName), score: rowScore(row.score)};
    });
    return {view: 'test_results', family, page: 1, ...(input.sortDate !== undefined ? {sortDate: input.sortDate} : {}), pageRowCount: list.length, rows};
  }
  if (!student) throw Error('tool_input_invalid');
  const rows: TestAttemptRow[] = [];
  let omitted = 0;
  for (const row of list) {
    if (rowKey(row.priNo, true) !== student.studentId) { omitted++; continue; }
    rows.push({paperNo: rowKey(row.pNo), testingNo: rowKey(row.testingNo), studentKey: student.studentId, classNo: rowKey(row.clsNo, true),
      title: rowText(row.testingName), applyDate: rowText(row.applyDate), score: rowScore(row.score)});
  }
  return {view: 'test_results', family, page: 1, student: {studentId: student.studentId, displayName: student.displayName}, period: {...input.period!}, pageRowCount: list.length,
    otherStudentRowsOmitted: omitted, rows};
}

export type LmsScheduleRead = AttendanceRead | StudyScheduleRead | CourseStudyManagerRead | TestResultsRead;
/** One page of the view, parsed fail closed for the principal's teacher key and the caller's scope. For view `test_results` with family da, fa or na, `student` is the
 * resolved student whose attempts the page may answer. */
export function parseLmsSchedule(input: LmsScheduleInput, html: string, parse: Parse, teacherKey: string, student?: {studentId: string; displayName: string}): LmsScheduleRead {
  if (input.view === 'test_results') return parseTestResults(input, html, parse, teacherKey, student);
  if (input.view === 'attendance') return parseAttendance(html, parse, teacherKey, {month: requestedMonth(input), ...(input.date || input.period ? {range: input.period ?? {from: input.date!, through: input.date!}} : {})});
  return input.view === 'study_schedule' ? parseStudySchedule(html, parse, teacherKey, !hasAllStudyScheduleFilters(input)) : parseCourseStudyManager(html, parse, teacherKey);
}
