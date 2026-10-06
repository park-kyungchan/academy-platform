import { CONTRACT_REVISION, type Role, type Student, type TeacherDecision } from '../domain/canonical.ts';
import { validLessonContext, type LessonContext, type TodayOverview, type StudentLessonDetail } from './lesson-views.ts';
export type ActionName = 'lookupStudent' | 'preparePrint' | 'recordTeacherDecision' | 'todayOverview' | 'studentLessonDetail';
export interface ActionParams {
  todayOverview: { context: LessonContext };
  studentLessonDetail: { studentId: string; context: LessonContext };
  lookupStudent: { studentId: string };
  preparePrint: { studentId: string; purpose: string };
  recordTeacherDecision: { candidateId: string; decision: 'accept' | 'dismiss'; rationale: string; expectedStatus: 'proposed'; expectedDataRevision: number };
}
export interface ActionOutputs {
  todayOverview: TodayOverview;
  studentLessonDetail: StudentLessonDetail;
  lookupStudent: Student;
  preparePrint: { studentId: string; text: string; physicalPrintTriggered: false; dataRevision: number };
  recordTeacherDecision: TeacherDecision;
}
export interface ActionRequest<K extends ActionName> {
  action: K; params: ActionParams[K]; contractsRevision: typeof CONTRACT_REVISION;
  academyId: string; idempotencyKey: string;
}
/** Must be supplied by a trusted identity boundary, never accepted from UI/tool arguments. */
export interface TrustedPrincipal { actorId: string; role: Role; academyIds: readonly string[]; }
export type FailureCode = 'INVALID_REQUEST' | 'REVISION_MISMATCH' | 'FORBIDDEN' | 'NOT_FOUND' | 'AMBIGUOUS' | 'PRECONDITION_FAILED' | 'IDEMPOTENCY_CONFLICT';
export type ActionResult<K extends ActionName> =
  | { ok: true; contractsRevision: typeof CONTRACT_REVISION; data: ActionOutputs[K]; evidenceIds: string[]; sideEffects: readonly string[]; replayed: boolean }
  | { ok: false; contractsRevision: typeof CONTRACT_REVISION; code: FailureCode; sideEffects: readonly []; evidenceIds: readonly [] };
export const ACTION_CONTRACTS = {
  todayOverview: { roles: ['viewer', 'teacher', 'administrator'], preconditions: ['explicit verified academy-local context', 'unique scoped references'], sideEffects: [], idempotency: 'scoped session replay' },
  studentLessonDetail: { roles: ['viewer', 'teacher', 'administrator'], preconditions: ['exactly one student in scope', 'explicit verified academy-local context', 'unique scoped references'], sideEffects: [], idempotency: 'scoped session replay' },
  lookupStudent: { roles: ['viewer', 'teacher', 'administrator'], preconditions: ['exactly one student in scope'], sideEffects: [], idempotency: 'scoped session replay' },
  preparePrint: { roles: ['teacher', 'administrator'], preconditions: ['exactly one student in scope'], sideEffects: [], idempotency: 'scoped session replay; no physical print' },
  recordTeacherDecision: { roles: ['teacher', 'administrator'], preconditions: ['one proposed candidate in scope', 'candidate evidence is available', 'no existing decision'], sideEffects: ['synthetic in-memory teacher decision'], idempotency: 'scoped session replay; reject changed payload' }
} as const;

const nonempty = (x: unknown): x is string => typeof x === 'string' && x.trim().length > 0 && x.length <= 2000;
const object = (x: unknown): x is Record<string, unknown> => typeof x === 'object' && x !== null && !Array.isArray(x);
const keys = (x: Record<string, unknown>, allowed: string[]) => Object.keys(x).length === allowed.length && allowed.every(k => Object.hasOwn(x, k));
export function isRequest(x: unknown): x is { [K in ActionName]: ActionRequest<K> }[ActionName] {
  if (!object(x) || !keys(x, ['action','params','contractsRevision','academyId','idempotencyKey'])) return false;
  if (!nonempty(x.academyId) || !nonempty(x.idempotencyKey) || typeof x.contractsRevision !== 'string' || !object(x.params)) return false;
  const p = x.params;
  switch (x.action) {
    case 'todayOverview': return keys(p, ['context']) && validLessonContext(p.context);
    case 'studentLessonDetail': return keys(p, ['studentId', 'context']) && nonempty(p.studentId) && validLessonContext(p.context);
    case 'lookupStudent': return keys(p, ['studentId']) && nonempty(p.studentId);
    case 'preparePrint': return keys(p, ['studentId','purpose']) && nonempty(p.studentId) && nonempty(p.purpose);
    case 'recordTeacherDecision': return keys(p, ['candidateId','decision','rationale','expectedStatus','expectedDataRevision']) && nonempty(p.candidateId) && nonempty(p.rationale) && (p.decision === 'accept' || p.decision === 'dismiss') && p.expectedStatus === 'proposed' && typeof p.expectedDataRevision === 'number' && Number.isSafeInteger(p.expectedDataRevision) && p.expectedDataRevision >= 0;
    default: return false;
  }
}
