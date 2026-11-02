/** Live prestudy (예습영상) list contracts, reverse-engineered 2026-11-02 from
 * the owner-authenticated CourseManageServlet pages:
 * - 검사 대기중: GET ?reqCmd=WebUnPreStudy, search = POST form1 (reqCmd=WebUnPreStudy)
 * - 검사 완료:   GET ?reqCmd=WebCtPreStudy, search = POST form1 (reqCmd=WebCtPreStudy)
 * - 요약:       GET ?reqCmd=TeacherPrestudySummary (observed page, no date form)
 * Waiting/completed rows carry prestudy_main(<key>); summary rows carry an AI
 * score literal, a star literal and report/video availability. Video URLs are
 * never retained: they embed a student key. Upload, list membership, AI score
 * and teacher review are distinct states; nothing here derives one from
 * another. Fictional fixtures only in tests. */

export const PRESTUDY_PATH = '/servlet/controller.coursemanage.CourseManageServlet';
export const PRESTUDY_SUMMARY_PATH = '/servlet/controller.coursemanage.CourseManageServlet?reqCmd=TeacherPrestudySummary';
export const TEXTBOOK_PROGRESS_PATH = '/servlet/controller.coursemanage.TextBookManageServlet?reqCmd=getTextBookProgress';

const clean = (value: string | null | undefined) => (value ?? '').replace(/\s+/g, ' ').trim();

function rejectExpired(html: string): void {
  if (/normal_session_error\.jsp|popup_session_error\.jsp|세션이 만료|<input\b[^>]*type\s*=\s*["']?password/i.test(html))
    throw Error('prestudy_authentication_required');
  if (html.length > 2_000_000) throw Error('prestudy_source_rejected');
}

export interface PrestudySearchForm {
  reqCmd: string; collegeKey: string; memberType: string; teacherKey: string;
}

/** The search form is the wire contract: hidden scope fields must be read
 * back from the live page, never reconstructed from caller input. */
export function parsePrestudySearchForm(html: string, parse: (html: string, mime: DOMParserSupportedType) => Document, expectedCmd: 'WebUnPreStudy' | 'WebCtPreStudy', teacherKey: string): PrestudySearchForm {
  rejectExpired(html);
  const doc = parse(html, 'text/html');
  const form = doc.querySelector('form[name="form1"]');
  if (!form) throw Error('prestudy_search_form_missing');
  const field = (name: string) => form.querySelector<HTMLInputElement>(`[name="${name}"]`)?.value;
  const reqCmd = field('reqCmd'), collegeKey = field('g_clg_no'), memberType = field('g_mem_type'), formTeacherKey = field('g_pri_no');
  if (reqCmd !== expectedCmd || !collegeKey || memberType === undefined || formTeacherKey !== teacherKey)
    throw Error('prestudy_search_form_drift');
  if (!form.querySelector('[name="student_name"]')) throw Error('prestudy_search_form_drift');
  return { reqCmd, collegeKey, memberType, teacherKey: formTeacherKey };
}

export interface PrestudyListRow {
  studentLabel: string; unitLabel: string; dateLiteral: string;
  /** Waiting list: source status literal (e.g. OK). Completed list: source rating literal. */
  statusLiteral: string; prestudyKey: string;
}

export function parsePrestudyList(html: string, parse: (html: string, mime: DOMParserSupportedType) => Document, expectedCmd: 'WebUnPreStudy' | 'WebCtPreStudy', teacherKey: string): PrestudyListRow[] {
  parsePrestudySearchForm(html, parse, expectedCmd, teacherKey);
  const doc = parse(html, 'text/html');
  const rows: PrestudyListRow[] = [];
  for (const row of doc.querySelectorAll('tr')) {
    const handler = [...row.querySelectorAll('[onclick]')].map(n => n.getAttribute('onclick') ?? '').join(' ');
    const key = /prestudy_main\(\s*([0-9]+)\s*\)/.exec(handler)?.[1];
    if (!key) continue;
    const cells = [...row.querySelectorAll(':scope > td')].map(n => clean(n.textContent));
    if (cells.length < 4 || !cells[0] || !cells[1]) throw Error('prestudy_row_unqualified');
    rows.push({ studentLabel: cells[0], unitLabel: cells[1], dateLiteral: cells[2] ?? '', statusLiteral: cells[3] ?? '', prestudyKey: key });
  }
  if (rows.length > 1000) throw Error('prestudy_row_unqualified');
  return rows;
}

export interface PrestudySummaryRow {
  studentLabel: string; unitLabel: string; dateLiteral: string;
  aiScoreLiteral: string; ratingLiteral: string; reportAvailable: boolean; videoAvailable: boolean;
}

export function parsePrestudySummary(html: string, parse: (html: string, mime: DOMParserSupportedType) => Document): PrestudySummaryRow[] {
  rejectExpired(html);
  const doc = parse(html, 'text/html');
  const rows: PrestudySummaryRow[] = [];
  for (const row of doc.querySelectorAll('tr')) {
    const handlers = [...row.querySelectorAll('[onclick]')].map(n => n.getAttribute('onclick') ?? '');
    const videoAvailable = handlers.some(h => /playVideo\(/.test(h));
    const reportAvailable = handlers.some(h => /openReport\(/.test(h));
    if (!videoAvailable && !reportAvailable) continue;
    const cells = [...row.querySelectorAll(':scope > td')].map(n => clean(n.textContent));
    if (cells.length < 5 || !cells[0] || !cells[1]) throw Error('prestudy_summary_row_unqualified');
    rows.push({
      studentLabel: cells[0], unitLabel: cells[1], dateLiteral: cells[2] ?? '',
      aiScoreLiteral: cells[3] ?? '', ratingLiteral: cells[4] ?? '', reportAvailable, videoAvailable,
    });
  }
  if (rows.length > 1000) throw Error('prestudy_summary_row_unqualified');
  return rows;
}

/** Exact-name match: the plain label, or the summary label with a source
 * suffix in parentheses (e.g. a grade literal). Never a substring match. */
export function prestudyRowsForStudent<T extends { studentLabel: string }>(rows: readonly T[], studentName: string): T[] {
  return rows.filter(r => r.studentLabel === studentName || r.studentLabel.startsWith(studentName + '('));
}

export interface TextbookBinding {
  courseKey: string; bookNo: string; studentKey: string; bookLabel: string;
}

/** getTextBookProgress renders one row per registered book; the wrong-note
 * button binds (courseKey, bookNo, studentKey) for that exact row. */
export function parseTextbookBindings(html: string, parse: (html: string, mime: DOMParserSupportedType) => Document, studentKey: string): TextbookBinding[] {
  rejectExpired(html);
  const doc = parse(html, 'text/html');
  const bindings = new Map<string, TextbookBinding>();
  for (const node of doc.querySelectorAll('[onclick]')) {
    const call = /createBookIncorrectPapers\(\s*['"]?([0-9]+)['"]?\s*,\s*['"]?([0-9]+)['"]?\s*,\s*['"]?([0-9]+)['"]?\s*\)/.exec(node.getAttribute('onclick') ?? '');
    if (!call || call[3] !== studentKey) continue;
    const row = node.closest('tr');
    const cells = row ? [...row.querySelectorAll(':scope > td')].map(n => clean(n.textContent)) : [];
    const bookLabel = cells[1] ?? cells.find(c => c && !/^\d+(\.\d+)?\s*%$/.test(c)) ?? '';
    const binding = { courseKey: call[1]!, bookNo: call[2]!, studentKey, bookLabel };
    bindings.set(JSON.stringify([binding.courseKey, binding.bookNo]), binding);
  }
  return [...bindings.values()];
}
