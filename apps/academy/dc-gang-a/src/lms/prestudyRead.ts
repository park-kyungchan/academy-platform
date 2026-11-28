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
export const PRESTUDY_VIEWS = ['videos', 'incomplete', 'teacher_summary'] as const;
export type PrestudyView = typeof PRESTUDY_VIEWS[number];
export const PRESTUDY_REPORT_PATH = '/app-ui/solution_common/prestudy/prestudy_ai_report.jsp';
export const PRESTUDY_SUMMARY_PATH = '/servlet/controller.coursemanage.CourseManageServlet?reqCmd=TeacherPrestudySummary';
export const TEXTBOOK_PROGRESS_PATH = '/servlet/controller.coursemanage.TextBookManageServlet?reqCmd=getTextBookProgress';

const clean = (value: string | null | undefined) => (value ?? '').replace(/\s+/g, ' ').trim();
/** A list or summary cell: a label, never a URL (R9 of specs/academy-mcp/lms-reads.md: a video URL embeds a student key). */
const cleanCell = (value: string | null | undefined) => clean((value ?? '').replace(/\bhttps?:\/\/[^\s"'<>]+/gi, ' ').replace(/(^|\s)\/\/[^\s"'<>]+/g, '$1 '));
/** R8 of specs/academy-mcp/lms-reads.md: one page is at most 2 MiB of UTF-8 and yields at most 1000 rows. */
export const PRESTUDY_MAX_PAGE_BYTES = 2 * 1024 * 1024;
export const PRESTUDY_MAX_ROWS = 1000;

function rejectExpired(html: string): void {
  if (Buffer.byteLength(html, 'utf8') > PRESTUDY_MAX_PAGE_BYTES) throw Error('prestudy_source_rejected');
  if (/normal_session_error\.jsp|popup_session_error\.jsp|세션이 만료|<input\b[^>]*type\s*=\s*["']?password/i.test(html))
    throw Error('prestudy_authentication_required');
}

/** The search form drifted from the contract. `scopeConflict` marks the teacher key that differs from the
 * principal's (a `scope_conflict` of lms-reads.md R5); any other difference is `source_drift`. */
export class PrestudyFormDrift extends Error {
  constructor(readonly scopeConflict: boolean) { super('prestudy_search_form_drift'); this.name = 'PrestudyFormDrift'; }
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
  if (reqCmd !== expectedCmd || !collegeKey || memberType === undefined) throw new PrestudyFormDrift(false);
  if (formTeacherKey !== teacherKey) throw new PrestudyFormDrift(true);
  if (!form.querySelector('[name="student_name"]')) throw new PrestudyFormDrift(false);
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
    const cells = [...row.querySelectorAll(':scope > td')].map(n => cleanCell(n.textContent));
    if (cells.length < 4 || !cells[0] || !cells[1]) throw Error('prestudy_row_unqualified');
    rows.push({ studentLabel: cells[0], unitLabel: cells[1], dateLiteral: cells[2] ?? '', statusLiteral: cells[3] ?? '', prestudyKey: key });
    if (rows.length > PRESTUDY_MAX_ROWS) throw Error('prestudy_source_rejected');
  }
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
    const cells = [...row.querySelectorAll(':scope > td')].map(n => cleanCell(n.textContent));
    if (cells.length < 5 || !cells[0] || !cells[1]) throw Error('prestudy_summary_row_unqualified');
    rows.push({
      studentLabel: cells[0], unitLabel: cells[1], dateLiteral: cells[2] ?? '',
      aiScoreLiteral: cells[3] ?? '', ratingLiteral: cells[4] ?? '', reportAvailable, videoAvailable,
    });
    if (rows.length > PRESTUDY_MAX_ROWS) throw Error('prestudy_source_rejected');
  }
  return rows;
}

/** Exact-name match: the plain label, or the summary label with a source
 * suffix in parentheses (e.g. a grade literal). Never a substring match. */
export function prestudyRowsForStudent<T extends { studentLabel: string }>(rows: readonly T[], studentName: string): T[] {
  return rows.filter(r => r.studentLabel === studentName || r.studentLabel.startsWith(studentName + '('));
}

export interface PrestudyDetail {
  prestudyKey: string; unitLabel: string | null; dateLiteral: string | null;
  analysisKey: string | null; encodingResult: string | null;
}

/** One uploaded-video detail record (WebPreStudy). The record is bound to
 * the exact student and requested record through its own form fields; the AI report is reachable
 * only through the rendered viewAiReport(analysis_key) handler. A missing
 * handler means no finished report is exposed for this record. */
export function parsePrestudyDetail(html: string, parse: (html: string, mime: DOMParserSupportedType) => Document, studentKey: string, prestudyKey: string): PrestudyDetail {
  rejectExpired(html);
  const doc = parse(html, 'text/html');
  const forms = doc.querySelectorAll('form[name="form1"]');
  if (forms.length !== 1) throw Error('prestudy_detail_unqualified');
  const form = forms[0];
  const field = (name: string): string | null => {
    const nodes = form.querySelectorAll<HTMLInputElement>(`input[name="${name}"]`);
    if (nodes.length > 1) throw Error('prestudy_detail_unqualified');
    const value = nodes[0]?.value;
    return value?.trim() ? value : null;
  };
  const studentField = field('student_pri_no');
  const sourceKey = field('prestudy_key');
  if (!studentField || !sourceKey || !/^[1-9][0-9]{0,15}$/.test(sourceKey)) throw Error('prestudy_detail_unqualified');
  if (studentField !== studentKey) throw Error('prestudy_detail_student_mismatch');
  if (sourceKey !== prestudyKey) throw Error('prestudy_detail_record_mismatch');
  let analysisKey: string | null = null;
  for (const node of doc.querySelectorAll('[onclick]')) {
    const match = /viewAiReport\(\s*'([^']+)'\s*\)|viewAiReport\(\s*"([^"]+)"\s*\)/.exec(node.getAttribute('onclick') ?? '');
    if (match) {
      const key = match[1] ?? match[2] ?? null;
      if (analysisKey !== null && analysisKey !== key) throw Error('prestudy_detail_unqualified');
      analysisKey = key;
    }
  }
  if (analysisKey !== null && !/^[1-9][0-9]{0,15}$/.test(analysisKey)) throw Error('prestudy_detail_unqualified');
  const encodingResult = clean(field('encoding_result') ?? '') || null;
  return { prestudyKey: sourceKey, unitLabel: field('sunit_name'), dateLiteral: field('create_datetime'), analysisKey, encodingResult };
}

export interface PrestudyReportBlock { heading: string | null; text: string }
export interface PrestudyReportSection { title: string; subtitle: string | null; blocks: PrestudyReportBlock[] }
export interface PrestudyReport { sections: PrestudyReportSection[]; scores: number[] }

function blockText(node: Node): string {
  let out = '';
  const walk = (n: Node): void => {
    if (n.nodeType === 3) { out += n.textContent ?? ''; return; }
    if (n.nodeType !== 1) return;
    const el = n as Element;
    if (el.tagName === 'BR') { out += '\n'; return; }
    if (el.tagName === 'SCRIPT' || el.tagName === 'STYLE') return;
    for (const child of n.childNodes) walk(child);
    if (/^(P|DIV|LI|TR|H[1-6])$/.test(el.tagName)) out += '\n';
  };
  walk(node);
  return out.split('\n').map(line => line.replace(/\s+/g, ' ').trim()).filter(Boolean).join('\n');
}

/** The AI report page is fully server-rendered: one card per section with
 * an h1 title, an optional h2 question, h3-headed narrative blocks and the
 * chart score array in an inline script. There is no transcript field on
 * this page; nothing here derives one. An invalid analysis key renders a
 * short cannot-generate page instead of the cards. */
export function parsePrestudyReport(html: string, parse: (html: string, mime: DOMParserSupportedType) => Document): PrestudyReport {
  rejectExpired(html);
  if (html.includes('리포트를 생성할 수 없습니다')) throw Error('prestudy_report_not_ready');
  const doc = parse(html, 'text/html');
  const cards = [...doc.querySelectorAll('div')].filter(el =>
    (el.getAttribute('class') ?? '').includes('max-w-5xl') && (el.getAttribute('class') ?? '').includes('rounded-2xl') && el.querySelector('h1'));
  const sections: PrestudyReportSection[] = [];
  let totalChars = 0;
  for (const card of cards) {
    const title = clean(card.querySelector('h1')?.textContent ?? '');
    if (!title) continue;
    const subtitle = clean(card.querySelector('h2')?.textContent ?? '') || null;
    const blocks: PrestudyReportBlock[] = [];
    let current: PrestudyReportBlock | null = null;
    for (const el of card.querySelectorAll('h3, div.text-sm, p')) {
      if (el.tagName === 'H3') { current = { heading: clean(el.textContent ?? '') || null, text: '' }; blocks.push(current); continue; }
      if (el.tagName === 'P' && card.querySelector('h3')) continue;
      const text = blockText(el);
      if (!text) continue;
      if (!current) { current = { heading: null, text: '' }; blocks.push(current); }
      current.text = current.text ? current.text + '\n' + text : text;
    }
    const kept = blocks.filter(b => b.text);
    totalChars += kept.reduce((sum, b) => sum + b.text.length, 0);
    if (kept.length) sections.push({ title, subtitle, blocks: kept });
  }
  if (!sections.length) throw Error('prestudy_report_unqualified');
  if (totalChars > 60000) throw Error('prestudy_report_source_rejected');
  const inline = [...doc.querySelectorAll('script:not([src])')].map(s => s.textContent ?? '').join('\n');
  const scoreObject = /\bscores\s*=\s*\{([^}]*)\}/.exec(inline)?.[1];
  const scoreArray = scoreObject ?? /\bscores\s*=\s*\[([^\]]*)\]/.exec(inline)?.[1] ?? '';
  const scores = [...scoreArray.matchAll(scoreObject ? /:\s*(-?\d+(?:\.\d+)?)/g : /-?\d+(?:\.\d+)?/g)]
    .map(m => Number(m[1] ?? m[0])).filter(Number.isFinite).slice(0, 20);
  return { sections, scores };
}

export interface TextbookBinding {
  courseKey: string; bookNo: string; studentKey: string; bookLabel: string;
}

/** A book label is a title, never a progress/date/number/button cell. This
 * predicate is the client half of the textbook-binding contract; the MCP
 * backend re-checks the same rule before exposing candidates. */
export function isTextbookBookLabel(value: string): boolean {
  const v = value.trim();
  if (v.length < 2 || v.length > 200) return false;
  if (/%/.test(v)) return false;
  if (/^[\d\s.,]+$/.test(v)) return false;
  if (/^\d{4}[-./]\d{1,2}[-./]\d{1,2}([^\d]|$)/.test(v) || /^\d{1,2}[-./]\d{1,2}[-./]\d{2,4}([^\d]|$)/.test(v)) return false;
  if (/오답노트|incorrect|wrong/i.test(v)) return false;
  return /[가-힣A-Za-z]/.test(v);
}

/** getTextBookProgress renders one row per registered book; the wrong-note
 * button binds (courseKey, bookNo, studentKey) for that exact row. */
interface GridCell { text: string; attrs: string[] }
/** Logical table grid: rowspan/colspan are expanded so a shifted physical
 * row (a carried cell from the row above) keeps its logical columns. */
function tableGrid(table: Element): { grid: GridCell[][]; headerRow: number } {
  const rows = [...table.querySelectorAll(':scope > tbody > tr, :scope > thead > tr, :scope > tr')];
  const grid: GridCell[][] = [];
  const carry = new Map<number, { cell: GridCell; left: number }>();
  let headerRow = -1;
  rows.forEach((tr, ri) => {
    const out: GridCell[] = [];
    let col = 0;
    const placeCarried = () => { while (carry.has(col)) { const c = carry.get(col)!; out[col] = c.cell; if (--c.left <= 0) carry.delete(col); col++; } };
    for (const node of tr.querySelectorAll(':scope > th, :scope > td')) {
      placeCarried();
      const attrs = [...node.querySelectorAll('input, img')].flatMap(n => [n.getAttribute('value') ?? '', n.getAttribute('title') ?? '', n.getAttribute('alt') ?? '']).map(clean).filter(Boolean);
      const cell: GridCell = { text: clean(node.textContent), attrs };
      const span = Math.max(1, Number(node.getAttribute('colspan') ?? 1) || 1);
      const down = Math.max(1, Number(node.getAttribute('rowspan') ?? 1) || 1);
      for (let k = 0; k < span; k++) { out[col] = cell; if (down > 1) carry.set(col, { cell, left: down - 1 }); col++; }
      if (node.tagName === 'TH' && headerRow < 0) headerRow = ri;
    }
    placeCarried();
    grid.push(out);
  });
  return { grid, headerRow };
}

export function parseTextbookBindings(html: string, parse: (html: string, mime: DOMParserSupportedType) => Document, studentKey: string): TextbookBinding[] {
  rejectExpired(html);
  const doc = parse(html, 'text/html');
  const bindings = new Map<string, TextbookBinding>();
  const grids = new Map<Element, ReturnType<typeof tableGrid>>();
  for (const node of doc.querySelectorAll('[onclick]')) {
    const call = /createBookIncorrectPapers\(\s*['"]?([0-9]+)['"]?\s*,\s*['"]?([0-9]+)['"]?\s*,\s*['"]?([0-9]+)['"]?\s*\)/.exec(node.getAttribute('onclick') ?? '');
    if (!call || call[3] !== studentKey) continue;
    const row = node.closest('tr');
    const table = row?.closest('table');
    if (!row || !table) throw Error('textbook_binding_source_unqualified');
    if (!grids.has(table)) grids.set(table, tableGrid(table));
    const { grid, headerRow } = grids.get(table)!;
    const tableRows = [...table.querySelectorAll(':scope > tbody > tr, :scope > thead > tr, :scope > tr')];
    const gridRow = grid[tableRows.indexOf(row)] ?? [];
    // Book column from the header row when named; the label may live in
    // cell text or in a nested input/image attribute.
    const headerCells = headerRow >= 0 ? (grid[headerRow] ?? []) : [];
    const bookCol = headerCells.findIndex(c => /교재|도서/.test(c.text));
    const at = (c: GridCell | undefined): string | undefined => {
      if (!c) return undefined;
      if (isTextbookBookLabel(c.text)) return c.text;
      return c.attrs.find(isTextbookBookLabel);
    };
    const fromHeader = bookCol >= 0 ? at(gridRow[bookCol]) : undefined;
    const qualified = gridRow.slice(1).map(at).filter((v): v is string => !!v);
    const uiWords = new Set(['생성', '보기', '확인', '닫기', '수정', '삭제', '출력', '인쇄', '검색', '등록', '저장', '취소']);
    const branded = qualified.find(c => /가우스|다빈치|개념원리/.test(c));
    const titled = qualified.find(c => !uiWords.has(c));
    const bookLabel = fromHeader ?? branded ?? titled ?? qualified[0];
    if (!bookLabel) throw Error('textbook_binding_source_unqualified');
    const binding = { courseKey: call[1]!, bookNo: call[2]!, studentKey, bookLabel };
    const key = JSON.stringify([binding.courseKey, binding.bookNo]);
    const prior = bindings.get(key);
    if (prior && prior.bookLabel !== bookLabel) throw Error('textbook_binding_source_unqualified');
    bindings.set(key, binding);
  }
  // No independently observed empty-progress contract is available. A generic
  // error document or rows for another student cannot establish zero books.
  if (!bindings.size) throw Error('textbook_binding_source_unqualified');
  return [...bindings.values()];
}
