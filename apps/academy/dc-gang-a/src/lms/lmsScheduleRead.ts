/** Read-only LMS schedule pages (specs/academy-mcp/lms-reads.md): attendance, the study schedule and the
 * course study manager. Each view is one GET of one servlet page; this module qualifies the caller's input
 * before any request, builds the request path, and parses the page fail closed:
 * - R4: a login page, a session-error page or a password input refuses with `authentication_required`;
 * - R5: a missing form, table or container refuses with `source_drift`, a page of another teacher or
 *   another month with `scope_conflict`;
 * - R8: a page over 2 MiB, or over 1000 rows, refuses with `source_rejected`;
 * - R9: a cell keeps its label only: no URL (so no video URL) and no cookie is ever returned.
 * The page shapes are pinned by the synthetic fixtures of tests/lms-reads.test.ts; the same HTML shapes are the
 * wire contract, never an inference from another page. */
import {isLoginOrSessionPage} from './courseBookReport';

export const ATTENDANCE_PATH = '/servlet/controller.cct.tutor.AttendanceServlet';
export const COURSE_MANAGE_PATH = '/servlet/controller.coursemanage.CourseManageServlet';
export const LMS_SCHEDULE_VIEWS = ['attendance', 'study_schedule', 'course_study_manager'] as const;
export type LmsScheduleView = typeof LMS_SCHEDULE_VIEWS[number];
export const MAX_LMS_PAGE_BYTES = 2 * 1024 * 1024;
export const MAX_LMS_ROWS = 1000;
export const MAX_LMS_PERIOD_DAYS = 31;
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
  if (isLoginOrSessionPage(html)) refuse('authentication_required');
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
}
/** The members each view takes besides `view`; any other member refuses before a request. */
export const LMS_SCHEDULE_ALLOWED: Readonly<Record<LmsScheduleView, readonly string[]>> = {
  attendance: ['date', 'period', 'groupKey', 'studentKey'],
  study_schedule: ['gradeNo', 'groupKey', 'studentKey', 'courseKey'],
  course_study_manager: ['gradeNo', 'semesterNo', 'levelNo', 'status'],
};
const MEMBERS = ['date', 'period', 'groupKey', 'studentKey', 'gradeNo', 'semesterNo', 'levelNo', 'courseKey', 'status'] as const;
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
  if (view !== 'attendance') return issues;
  if (input.date !== undefined && input.period !== undefined) issues.push({path: ['period'], message: 'date_or_period_not_both'});
  if (input.date !== undefined && !isCalendarDate(input.date)) issues.push({path: ['date'], message: 'date_not_a_calendar_date'});
  if (input.period !== undefined) {
    const p = input.period as {from?: unknown; through?: unknown} | null;
    const keys = p && typeof p === 'object' ? Object.keys(p).sort().join(',') : '';
    if (keys !== 'from,through') issues.push({path: ['period'], message: 'period_from_through_required'});
    else if (!isCalendarDate(p!.from) || !isCalendarDate(p!.through)) {
      if (!isCalendarDate(p!.from)) issues.push({path: ['period', 'from'], message: 'date_not_a_calendar_date'});
      if (!isCalendarDate(p!.through)) issues.push({path: ['period', 'through'], message: 'date_not_a_calendar_date'});
    } else {
      const span = dayNumber(p!.through as string) - dayNumber(p!.from as string) + 1;
      if (span < 1) issues.push({path: ['period'], message: 'period_from_after_through'});
      else if (span > MAX_LMS_PERIOD_DAYS) issues.push({path: ['period'], message: 'period_over_31_days'});
      // One GET of AttendanceServlet returns one month; a longer span would silently omit rows.
      else if ((p!.from as string).slice(0, 7) !== (p!.through as string).slice(0, 7)) issues.push({path: ['period'], message: 'attendance_period_crosses_month'});
    }
  }
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

/** The request path of one view: the servlet command and, in the page's own parameter names, the filters. */
export function lmsScheduleRequestPath(input: LmsScheduleInput): string {
  const q = new URLSearchParams();
  const add = (name: string, value: string | undefined) => { if (value !== undefined) q.set(name, value); };
  if (input.view === 'attendance') {
    q.set('p_process', 'Main');
    const month = requestedMonth(input);
    if (month) { q.set('ym', month.replace('-', '')); q.set('sdate', month); }
    add('grp_seq', input.groupKey); add('stu_obj', input.studentKey);
    return ATTENDANCE_PATH + '?' + q;
  }
  if (input.view === 'study_schedule') {
    q.set('reqCmd', 'StudySchedule');
    add('grade_no', input.gradeNo); add('cls_no', input.groupKey); add('student_no', input.studentKey); add('course', input.courseKey);
    return COURSE_MANAGE_PATH + '?' + q;
  }
  q.set('reqCmd', 'CourseStudyManager');
  add('grade_no', input.gradeNo); add('semester_no', input.semesterNo); add('level_no', input.levelNo); add('status', input.status);
  if ([input.gradeNo, input.semesterNo, input.levelNo, input.status].some(v => v !== undefined)) q.set('search', '1');
  return COURSE_MANAGE_PATH + '?' + q;
}

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
export interface StudyScheduleRead {view: 'study_schedule'; columns: string[]; rows: LmsTableRow[]; grades: LmsOption[]; classes: LmsOption[]; students: LmsOption[]; courses: LmsOption[]}
export interface CourseStudyManagerRead {view: 'course_study_manager'; columns: string[]; rows: LmsTableRow[]; grades: LmsOption[]; semesters: LmsOption[]; levels: LmsOption[]; statuses: LmsOption[]}

const SCHEDULE_CONTROLS = {
  study_schedule: {command: 'StudySchedule', selects: ['grade_no', 'cls_no', 'student_no', 'course']},
  course_study_manager: {command: 'CourseStudyManager', selects: ['grade_no', 'semester_no', 'level_no', 'status']},
} as const;

function parseContentList(view: 'study_schedule' | 'course_study_manager', html: string, parse: Parse, teacherKey: string) {
  assertLmsPageSize(html);
  assertNotLoginPage(html);
  const doc = parse(html, 'text/html');
  const spec = SCHEDULE_CONTROLS[view];
  const form = doc.querySelector('form#form1');
  if (!form) refuse('source_drift');
  const command = form!.querySelector('input[name="reqCmd"]');
  if (!command || command.getAttribute('value') !== spec.command) refuse('source_drift');
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
  if (tables.length === 0) refuse('source_drift');
  const columns: string[] = [];
  const rows: LmsTableRow[] = [];
  for (const table of tables) {
    const all = directRows(table);
    const headerRow = all.find(r => r.querySelector(':scope > th'));
    const headers = headerRow ? [...headerRow.querySelectorAll(':scope > th')].map((th, i) => lmsCellText(th.textContent) || `column${i + 1}`) : [];
    const unique = headers.map((h, i) => headers.indexOf(h) === i ? h : `${h}#${i + 1}`);
    if (columns.length === 0) columns.push(...unique);
    for (const row of all) {
      if (row === headerRow || !row.querySelector(':scope > td')) continue;
      if (isEmptyStateRow(row)) continue;
      const cells = cellsOf(row);
      const out: LmsTableRow = {};
      cells.forEach((text, i) => { out[unique[i] ?? `column${i + 1}`] = text; });
      rows.push(out);
      if (rows.length > MAX_LMS_ROWS) refuse('source_rejected');
    }
  }
  const options = (name: string) => optionsOf(selects.get(name)!);
  return {columns, rows, options};
}

export function parseStudySchedule(html: string, parse: Parse, teacherKey: string): StudyScheduleRead {
  const {columns, rows, options} = parseContentList('study_schedule', html, parse, teacherKey);
  return {view: 'study_schedule', columns, rows, grades: options('grade_no'), classes: options('cls_no'), students: options('student_no'), courses: options('course')};
}
export function parseCourseStudyManager(html: string, parse: Parse, teacherKey: string): CourseStudyManagerRead {
  const {columns, rows, options} = parseContentList('course_study_manager', html, parse, teacherKey);
  return {view: 'course_study_manager', columns, rows, grades: options('grade_no'), semesters: options('semester_no'), levels: options('level_no'), statuses: options('status')};
}

export type LmsScheduleRead = AttendanceRead | StudyScheduleRead | CourseStudyManagerRead;
/** One page of the view, parsed fail closed for the principal's teacher key and the caller's scope. */
export function parseLmsSchedule(input: LmsScheduleInput, html: string, parse: Parse, teacherKey: string): LmsScheduleRead {
  if (input.view === 'attendance') return parseAttendance(html, parse, teacherKey, {month: requestedMonth(input), ...(input.date || input.period ? {range: input.period ?? {from: input.date!, through: input.date!}} : {})});
  return input.view === 'study_schedule' ? parseStudySchedule(html, parse, teacherKey) : parseCourseStudyManager(html, parse, teacherKey);
}
