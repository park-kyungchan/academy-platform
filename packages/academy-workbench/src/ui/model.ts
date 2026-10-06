import type { HumanUIPort } from '../interfaces/ports.ts';
import { CONTRACT_REVISION } from '../domain/canonical.ts';
import type { LessonContext, SyntheticLesson } from '../contracts/lesson-views.ts';

export function reviewLabel(lesson: SyntheticLesson): string {
  if (!lesson.reviewContext) return '조회 정보 미확인';
  if (lesson.reviewContext.stale) return '오래된 자료';
  if (lesson.reviewContext.coverage === 'partial') return '부분 조회';
  return '개별 확인 필요';
}
/** All app actions cross the same bound service; no source data is queried directly. */
export class WorkbenchModel {
  private sequence = 0;
  selectedStudent: string | null = null;
  draft: { studentId: string; candidateId: string; revision: number; key: string } | null = null;
  readonly decisions = new Map<string, string>();
  constructor(private port: HumanUIPort, readonly context: LessonContext) {}
  private base() { return { academyId: 'synthetic-academy', contractsRevision: CONTRACT_REVISION, idempotencyKey: `ui-${++this.sequence}` }; }
  overview() { return this.port.invoke({ ...this.base(), action: 'todayOverview', params: { context: this.context } }); }
  select(studentId: string | null) {
    this.cancel(); this.selectedStudent = studentId;
    return studentId === null ? null : this.port.invoke({ ...this.base(), action: 'studentLessonDetail', params: { studentId, context: this.context } });
  }
  preview(candidateId: string) {
    if (!this.selectedStudent) throw new Error('학생을 먼저 선택하세요.');
    const result = this.port.invoke({ ...this.base(), action: 'preparePrint', params: { studentId: this.selectedStudent, purpose: '합성 강사 판단 미리보기' } });
    if (result.ok) this.draft = { studentId: this.selectedStudent, candidateId, revision: result.data.dataRevision, key: `decision-${++this.sequence}` };
    return result;
  }
  cancel() { this.draft = null; }
  confirm(decision: 'accept' | 'dismiss', rationale: string) {
    if (!this.draft || this.draft.studentId !== this.selectedStudent || !rationale.trim()) return null;
    const draft = this.draft;
    const result = this.port.invoke({ ...this.base(), idempotencyKey: draft.key, action: 'recordTeacherDecision', params: { candidateId: draft.candidateId, decision, rationale, expectedStatus: 'proposed', expectedDataRevision: draft.revision } });
    if (result.ok) { this.decisions.set(draft.studentId, decision === 'accept' ? '강사 검토 채택' : '강사 검토 제외'); this.cancel(); }
    return result;
  }
}
