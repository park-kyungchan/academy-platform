import { CONTRACT_REVISION, type CanonicalSnapshot } from '../domain/canonical.ts';
import { ACTION_CONTRACTS, isRequest, type ActionName, type ActionOutputs, type ActionRequest, type ActionResult, type FailureCode, type TrustedPrincipal } from '../contracts/actions.ts';
import type { SyntheticLesson } from '../contracts/lesson-views.ts';
import type { StudentBookMap } from '../contracts/curriculum.ts';
import { validateBookMap } from '../domain/curriculum.ts';
import type { StudentReadRepository } from '../contracts/student-history.ts';

const stable = (value: unknown): unknown => Array.isArray(value) ? value.map(stable) : value !== null && typeof value === 'object' ? Object.fromEntries(Object.entries(value).sort(([a], [b]) => a.localeCompare(b)).map(([key, item]) => [key, stable(item)])) : value;

/** Synthetic-only synchronous reference boundary. No persistence or authentication implementation. */
export class SyntheticApplicationService {
  private readonly snapshot: CanonicalSnapshot;
  private readonly lessons: SyntheticLesson[];
  private readonly books: StudentBookMap[];
  private readonly dataRevisions = new Map<string, number>();
  private readonly replay = new Map<string, { fingerprint: string; result: ActionResult<ActionName> }>();
  constructor(snapshot: CanonicalSnapshot, lessons: SyntheticLesson[] = [], books: StudentBookMap[] = [], private readonly studentRepository?: StudentReadRepository) {
    if (snapshot.contractsRevision !== CONTRACT_REVISION || snapshot.evidence.some(e => e.source !== 'synthetic')) throw new Error('Synthetic fixture and matching revision required');
    this.snapshot = structuredClone(snapshot);
    this.lessons = structuredClone(lessons);
    books.forEach(validateBookMap);
    this.books = structuredClone(books);
  }
  execute<K extends ActionName>(principal: TrustedPrincipal, request: ActionRequest<K>): ActionResult<K>;
  execute(principal: TrustedPrincipal, request: unknown): ActionResult<ActionName>;
  execute(principal: TrustedPrincipal, raw: unknown): ActionResult<ActionName> {
    const fail = (code: FailureCode): ActionResult<ActionName> => ({ ok: false, contractsRevision: CONTRACT_REVISION, code, sideEffects: [], evidenceIds: [] });
    if (!isRequest(raw)) return fail('INVALID_REQUEST');
    if (raw.contractsRevision !== CONTRACT_REVISION) return fail('REVISION_MISMATCH');
    if (!principal || !principal.actorId?.trim() || !Array.isArray(principal.academyIds) || !principal.academyIds.includes(raw.academyId) || !(ACTION_CONTRACTS[raw.action].roles as readonly string[]).includes(principal.role)) return fail('FORBIDDEN');
    const key = JSON.stringify([principal.actorId, raw.academyId, raw.action, raw.idempotencyKey]);
    // Sorted fields make transport property order irrelevant to idempotency.
    const fingerprint = JSON.stringify(stable(raw.params));
    const cached = this.replay.get(key);
    if (cached) return cached.fingerprint === fingerprint ? { ...structuredClone(cached.result), ...(cached.result.ok ? { replayed: true } : {}) } : fail('IDEMPOTENCY_CONFLICT');
    let data: ActionOutputs[ActionName];
    let evidenceIds: string[] = [];
    if (raw.action === 'todayOverview' || raw.action === 'studentLessonDetail') {
      const context = raw.params.context;
      const detailStudentId = raw.action === 'studentLessonDetail' ? raw.params.studentId : undefined;
      const students = this.snapshot.students.filter(s => s.academyId === raw.academyId);
      if (raw.action === 'studentLessonDetail') {
        const matches = students.filter(s => s.id === detailStudentId);
        if (matches.length !== 1) return fail(matches.length ? 'AMBIGUOUS' : 'NOT_FOUND');
      }
      const lessons = this.lessons.filter(l => l.academyId === raw.academyId && l.context.date === context.date && (raw.action !== 'studentLessonDetail' || l.studentId === raw.params.studentId));
      if (new Set(lessons.map(l => l.id)).size !== lessons.length) return fail('PRECONDITION_FAILED');
      for (const lesson of lessons) {
        if (lesson.context.academyTimezone !== context.academyTimezone || lesson.context.siteTimezone !== context.siteTimezone || lesson.observations.assignmentId !== lesson.assignmentId) return fail('PRECONDITION_FAILED');
        if (students.filter(s => s.id === lesson.studentId).length !== 1) return fail('PRECONDITION_FAILED');
        const assignments = this.snapshot.assignments.filter(a => a.academyId === raw.academyId && a.id === lesson.assignmentId);
        if (assignments.length !== 1 || this.snapshot.enrollments.filter(e => e.academyId === raw.academyId && e.id === assignments[0]!.enrollmentId && e.studentId === lesson.studentId && e.status === 'active').length !== 1) return fail('PRECONDITION_FAILED');
        if (lesson.evidenceIds.some(id => this.snapshot.evidence.filter(e => e.academyId === raw.academyId && e.id === id).length !== 1)) return fail('PRECONDITION_FAILED');
        evidenceIds.push(...lesson.evidenceIds);
      }
      data = raw.action === 'todayOverview'
        ? { context, rows: lessons.map(l => ({ lessonId: l.id, studentId: l.studentId, displayName: students.find(s => s.id === l.studentId)!.displayName, localStartTime: l.localStartTime, evidenceIds: l.evidenceIds, questionsToCheck: l.questionsToCheck, preparation: l.preparation })) }
        : { context, studentId: raw.params.studentId, displayName: students.find(s => s.id === detailStudentId)!.displayName, lessons,
          videoRequirements: this.snapshot.videoRequirements.filter(r => r.academyId === raw.academyId && lessons.some(l => l.assignmentId === r.assignmentId)),
          problemRequirements: this.snapshot.problemSectionRequirements.filter(r => r.academyId === raw.academyId && lessons.some(l => l.assignmentId === r.assignmentId)),
          evidence: this.snapshot.evidence.filter(e => e.academyId === raw.academyId && evidenceIds.includes(e.id)),
          candidates: this.snapshot.clinicCandidates.filter(c => c.academyId === raw.academyId && c.studentId === detailStudentId),
          books: [...(this.studentRepository?.readStudentBooks(raw.academyId, raw.params.studentId) ?? this.books.filter(b => b.academyId === raw.academyId && b.studentId === detailStudentId))],
          timeline: [...(this.studentRepository?.readStudentHistory(raw.academyId, raw.params.studentId) ?? [])] };
    } else if (raw.action === 'lookupStudent' || raw.action === 'preparePrint') {
      const students = this.snapshot.students.filter(s => s.academyId === raw.academyId && s.id === raw.params.studentId);
      if (students.length !== 1) return fail(students.length ? 'AMBIGUOUS' : 'NOT_FOUND');
      const student = students[0]!;
      data = raw.action === 'lookupStudent' ? student : { studentId: student.id, text: `${raw.params.purpose}\n${student.displayName}`, physicalPrintTriggered: false, dataRevision: this.dataRevisions.get(raw.academyId) ?? 0 };
    } else {
      if (raw.params.expectedDataRevision !== (this.dataRevisions.get(raw.academyId) ?? 0)) return fail('PRECONDITION_FAILED');
      const candidates = this.snapshot.clinicCandidates.filter(c => c.academyId === raw.academyId && c.id === raw.params.candidateId);
      if (candidates.length !== 1) return fail(candidates.length ? 'AMBIGUOUS' : 'NOT_FOUND');
      const candidate = candidates[0]!;
      if (candidate.status !== raw.params.expectedStatus || !candidate.gradingEvidenceIds.length || this.snapshot.teacherDecisions.some(d => d.academyId === raw.academyId && d.candidateId === candidate.id)) return fail('PRECONDITION_FAILED');
      if (this.snapshot.students.filter(s => s.academyId === raw.academyId && s.id === candidate.studentId).length !== 1) return fail('PRECONDITION_FAILED');
      for (const id of candidate.gradingEvidenceIds) {
        const grades = this.snapshot.gradingEvidence.filter(g => g.academyId === raw.academyId && g.id === id);
        if (grades.length !== 1 || grades[0]!.outcome === 'unknown') return fail('PRECONDITION_FAILED');
        const grade = grades[0]!;
        const assignments = this.snapshot.assignments.filter(a => a.academyId === raw.academyId && a.id === grade.assignmentId);
        if (assignments.length !== 1 || this.snapshot.enrollments.filter(e => e.academyId === raw.academyId && e.id === assignments[0]!.enrollmentId && e.studentId === candidate.studentId && e.status === 'active').length !== 1) return fail('PRECONDITION_FAILED');
        const requirements = [...this.snapshot.videoRequirements, ...this.snapshot.problemSectionRequirements].filter(r => r.academyId === raw.academyId && r.id === grade.requirementId && r.assignmentId === grade.assignmentId);
        if (requirements.length !== 1) return fail('PRECONDITION_FAILED');
        const evidence = this.snapshot.evidence.filter(e => e.academyId === raw.academyId && e.id === grades[0]!.sourceEvidenceId);
        if (evidence.length !== 1) return fail('PRECONDITION_FAILED');
        evidenceIds.push(evidence[0]!.id);
      }
      data = { id: `synthetic-decision-${this.snapshot.teacherDecisions.length + 1}`, academyId: raw.academyId, candidateId: candidate.id, teacherId: principal.actorId, decision: raw.params.decision, rationale: raw.params.rationale };
      this.snapshot.teacherDecisions.push(data);
      this.dataRevisions.set(raw.academyId, raw.params.expectedDataRevision + 1);
    }
    const result: ActionResult<ActionName> = { ok: true, contractsRevision: CONTRACT_REVISION, data: structuredClone(data), evidenceIds: [...new Set(evidenceIds)], sideEffects: ACTION_CONTRACTS[raw.action].sideEffects, replayed: false };
    this.replay.set(key, { fingerprint, result: structuredClone(result) });
    return result;
  }
}
