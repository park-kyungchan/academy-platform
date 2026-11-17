import {readFailureResult} from './read-failure.ts';
import {z} from 'zod';
import {dailyRecordDateSchema} from './dailyrecord-context.ts';

/** Name → native student-key resolver (G2). Exact display-name match only.
 * Implemented by the native/source owner through the search port; this
 * module owns the matching rules. Read-only, per-request, zero writes. */

export const studentResolveInputSchema = z.object({
  purpose: z.string().min(1).max(256),
  currentRequestRef: z.string().min(1).max(256),
  displayName: z.string().min(1).max(200),
  lessonDate: dailyRecordDateSchema.optional(),
}).strict();

export type StudentResolveInput = z.infer<typeof studentResolveInputSchema>;

const studentSearchRowSchema = z.object({
  studentKey: z.string().regex(/^[1-9][0-9]{0,15}$/),
  studentName: z.string().min(1).max(200),
  // The DT/ZT history page that found the name after the sort_date '1' page.
  fixedSourceFilter: z.object({sort_date: z.enum(['11', '12', '13', '14', '15', '16'])}).strict().optional(),
}).strict();

export type StudentSearchRow = z.infer<typeof studentSearchRowSchema>;

export interface StudentNameSearchPort {
  searchByName(exactName: string): Promise<unknown>;
}

export type StudentResolveResult =
  | {status: 'resolved'; studentKey: string; displayName: string; fixedSourceFilter?: {sort_date: string}}
  | {status: 'unavailable'; reasonCode: string};

const MAX_ROWS = 200;

const unavailable = (reasonCode: string): StudentResolveResult => ({status: 'unavailable', reasonCode});

export async function resolveStudentByName(
  displayName: unknown,
  port: StudentNameSearchPort | null,
): Promise<StudentResolveResult> {
  if (typeof displayName !== 'string') return unavailable('student_name_invalid');
  const name = displayName.trim();
  if (!name || name.length > 200) return unavailable('student_name_invalid');
  if (!port) return unavailable('student_resolve_backend_not_configured');
  let rows: StudentSearchRow[];
  try {
    rows = z.array(studentSearchRowSchema).max(MAX_ROWS + 1).parse(await port.searchByName(name));
  } catch (error) {
    return {...unavailable('student_resolve_search_unqualified_no_automatic_retry'),...readFailureResult(error,'student_resolve_search_unqualified_no_automatic_retry')};
  }
  if (rows.length > MAX_ROWS) return unavailable('student_resolve_search_unqualified_no_automatic_retry');
  if (rows.some(row => row.studentName !== name)) return unavailable('student_name_scope_conflict');
  if (!rows.length) return unavailable('student_name_not_found');
  if (new Set(rows.map(row => row.studentKey)).size > 1) return unavailable('student_name_ambiguous');
  // Rows of one search come from one page; a mixed page identity is not qualified.
  if (new Set(rows.map(row => row.fixedSourceFilter?.sort_date ?? null)).size > 1) return unavailable('student_resolve_search_unqualified_no_automatic_retry');
  const filter = rows[0]!.fixedSourceFilter;
  return {status: 'resolved', studentKey: rows[0]!.studentKey, displayName: name, ...(filter ? {fixedSourceFilter: {sort_date: filter.sort_date}} : {})};
}
