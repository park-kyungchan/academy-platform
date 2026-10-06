import type { AssignmentObservations } from './collection.ts';
import type { VideoRequirement, ProblemSectionRequirement, Evidence, ClinicCandidate } from '../domain/canonical.ts';
import type { StudentBookMap } from './curriculum.ts';
import type { StudentHistoryEntry } from './student-history.ts';

/** Synthetic read contracts only; callers supply the academy-local calendar date. */
export interface LessonContext {
  date: string;
  academyTimezone: string;
  siteTimezone: string | 'unknown';
}
export interface LessonMaterial {
  textbookEdition: string;
  unit: string;
  section: string;
  problemRange: { first: number; last: number };
  videoVersion: string;
}
export interface SyntheticLesson {
  id: string;
  academyId: string;
  studentId: string;
  context: LessonContext;
  localStartTime: string;
  assignmentId: string;
  material: LessonMaterial;
  observations: AssignmentObservations;
  evidenceIds: string[];
  questionsToCheck: string[];
  preparation: string[];
  history: { assignmentId: string; note: string }[];
  reviewContext?: { assignedAt: string; observedAt: string; coverage: 'partial' | 'all'; stale: boolean; reasons: string[] };
}
export interface TodayOverview {
  context: LessonContext;
  rows: { lessonId: string; studentId: string; displayName: string; localStartTime: string;
    evidenceIds: string[]; questionsToCheck: string[]; preparation: string[] }[];
}
export interface StudentLessonDetail {
  context: LessonContext;
  studentId: string;
  displayName: string;
  lessons: SyntheticLesson[];
  videoRequirements: VideoRequirement[];
  problemRequirements: ProblemSectionRequirement[];
  evidence: Evidence[];
  candidates: ClinicCandidate[];
  books: StudentBookMap[];
  timeline: StudentHistoryEntry[];
}

export function validLessonContext(value: unknown): value is LessonContext {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const c = value as Record<string, unknown>;
  if (Object.keys(c).length !== 3 || !['date', 'academyTimezone', 'siteTimezone'].every(k => Object.hasOwn(c, k))) return false;
  if (typeof c.date !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(c.date)) return false;
  const date = new Date(`${c.date}T00:00:00Z`);
  if (!Number.isFinite(date.getTime()) || date.toISOString().slice(0, 10) !== c.date) return false;
  const timezone = (v: unknown) => {
    if (typeof v !== 'string' || !v.trim() || v === 'unknown') return false;
    try { new Intl.DateTimeFormat('en', { timeZone: v }); return true; } catch { return false; }
  };
  return timezone(c.academyTimezone) && (c.siteTimezone === 'unknown' || timezone(c.siteTimezone));
}
