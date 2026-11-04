/** Merged textbook section index basis.
 *
 * The wrong-answer-note query (getWrongAnswerNoteResult) is scoped by exact
 * course/book/student, but it only lists sections with at least one wrong
 * item: an all-correct (100-point) section never appears there. The student
 * paper search (UserBySearchTestResult) lists every observed attempt for the
 * exact student regardless of wrong items, but its rows carry no course/book
 * key. The only sound join is therefore title-based and book-anchored:
 *
 * - the wrong-note titles for this exact course/book define the section
 *   title template: leading bracket tokens such as
 *   `[필수예제] [가우스] 5-4 대분수의 뺄셈 - 신지우`, where the first token is
 *   the activity observed for this book and the second token is the book
 *   label observed for this book, plus the trailing ` - <studentName>`
 *   suffix when every wrong-note title carries it;
 * - a paper-search row joins this book only when its testingName matches
 *   that template exactly (activity token, book token, suffix). No title is
 *   ever guessed and no attempt key is ever invented: paper rows supply
 *   their own testingNo/pNo, and the SingleUser detail read afterwards
 *   independently requires its sub_title to equal the requested title.
 *
 * Paper-search coverage stays one observed page under the source form's
 * period, so a merged index is never complete-history coverage. */
export interface TextbookSectionRow {
  sourceTitle: string;
  paperKey: string;
  attemptKey: string;
  sourceDateLiteral: string | null;
  sources: readonly ('wrong_answer_note' | 'student_paper_search')[];
}
export interface TextbookPaperSearchRow {
  testingName: string;
  paperKey: string;
  attemptKey: string;
  applyDateLiteral: string | null;
}
export interface TextbookSectionTitlePattern {
  /** Observed (activity, book) bracket pairs only. A pair that never
   * appeared together in this book's wrong-note titles does not join,
   * even when both tokens appeared separately with other partners. */
  pairs: readonly {activity: string; book: string}[];
  studentSuffix: string | null;
}
function bracketTokens(title: string): string[] {
  const tokens: string[] = [];
  let rest = title;
  for (;;) {
    const match = /^\[([^\]]+)\]\s*/.exec(rest);
    if (!match) return tokens;
    tokens.push(match[1]!);
    rest = rest.slice(match[0].length);
  }
}
/** Derive the book's section-title template from its wrong-note titles.
 * Returns null when the template cannot anchor paper rows to this book
 * (no titles, or titles without an activity+book bracket pair). */
export function deriveTextbookSectionTitlePattern(
  wrongNoteTitles: readonly string[],
  studentName: string,
): TextbookSectionTitlePattern | null {
  if (!wrongNoteTitles.length) return null;
  const tokenLists = wrongNoteTitles.map(bracketTokens);
  if (tokenLists.some(tokens => tokens.length < 2)) return null;
  const seen = new Set<string>();
  const pairs: {activity: string; book: string}[] = [];
  for (const tokens of tokenLists) {
    const key = tokens[0]! + '\0' + tokens[1]!;
    if (seen.has(key)) continue;
    seen.add(key);
    pairs.push({activity: tokens[0]!, book: tokens[1]!});
  }
  const suffix = ` - ${studentName}`;
  return {
    pairs,
    studentSuffix: wrongNoteTitles.every(title => title.endsWith(suffix)) ? suffix : null,
  };
}
export function isTextbookSectionPaperTitle(title: string, pattern: TextbookSectionTitlePattern): boolean {
  const tokens = bracketTokens(title);
  if (tokens.length < 2) return false;
  if (!pattern.pairs.some(pair => pair.activity === tokens[0]! && pair.book === tokens[1]!)) return false;
  if (pattern.studentSuffix !== null && !title.endsWith(pattern.studentSuffix)) return false;
  // A bare template echo with no section-specific remainder is not a section.
  const remainder = title.replace(/^(\[[^\]]+\]\s*)+/, '').trim();
  return remainder.length > 0 && remainder !== (pattern.studentSuffix ?? '').trim();
}
/** Merge wrong-note rows with template-matching paper-search rows for the
 * exact student. Wrong-note rows always stay; a shared attempt key must
 * carry the same title in both sources or the merge refuses to guess. */
export function mergeTextbookSectionRows(
  wrongNoteRows: readonly Omit<TextbookSectionRow, 'sources'>[],
  paperRows: readonly TextbookPaperSearchRow[],
  pattern: TextbookSectionTitlePattern | null,
): TextbookSectionRow[] {
  const merged: TextbookSectionRow[] = wrongNoteRows.map(row => ({...row, sources: ['wrong_answer_note'] as const}));
  const byAttempt = new Map(merged.map(row => [row.attemptKey, row] as const));
  for (const paper of paperRows) {
    const existing = byAttempt.get(paper.attemptKey);
    if (existing) {
      if (existing.sourceTitle !== paper.testingName) throw new Error('textbook_section_title_conflict_for_attempt');
      if (!existing.sources.includes('student_paper_search')) {
        const index = merged.indexOf(existing);
        merged[index] = {...existing, sources: ['wrong_answer_note', 'student_paper_search']};
        byAttempt.set(paper.attemptKey, merged[index]!);
      }
      continue;
    }
    if (!pattern || !isTextbookSectionPaperTitle(paper.testingName, pattern)) continue;
    const row: TextbookSectionRow = {
      sourceTitle: paper.testingName,
      paperKey: paper.paperKey,
      attemptKey: paper.attemptKey,
      sourceDateLiteral: paper.applyDateLiteral,
      sources: ['student_paper_search'],
    };
    merged.push(row);
    byAttempt.set(row.attemptKey, row);
  }
  return merged;
}
export const TEXTBOOK_PAPER_SEARCH_COVERAGE = 'one_observed_student_search_page_not_complete_history' as const;
export const TEXTBOOK_SECTION_NOT_IN_EITHER = 'textbook_section_not_in_either_observed_source' as const;
