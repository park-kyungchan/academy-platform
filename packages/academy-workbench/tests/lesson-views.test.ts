import { expect, test } from 'bun:test';
import { SyntheticApplicationService } from '../src/application/service.ts';
import { bindTrustedSession, type HumanUIPort, type AgentSemanticToolsPort } from '../src/interfaces/ports.ts';
import { syntheticSnapshot } from '../fixtures/synthetic.ts';
import { syntheticLessons } from '../fixtures/lessons.ts';
import { CONTRACT_REVISION } from '../src/domain/canonical.ts';
import type { ActionRequest, TrustedPrincipal } from '../src/contracts/actions.ts';

const teacher: TrustedPrincipal = { actorId: 'synthetic-teacher', role: 'teacher', academyIds: ['synthetic-academy'] };
const overview: ActionRequest<'todayOverview'> = { action: 'todayOverview', academyId: 'synthetic-academy', contractsRevision: CONTRACT_REVISION, idempotencyKey: 'today', params: { context: syntheticLessons[0]!.context } };
const detail: ActionRequest<'studentLessonDetail'> = { ...overview, action: 'studentLessonDetail', params: { context: overview.params.context, studentId: 'synthetic-student-1' } };
const service = () => new SyntheticApplicationService(syntheticSnapshot, syntheticLessons);

test('T01 overview to detail selects opaque ID despite duplicate names through shared ports', () => {
  const snapshot = structuredClone(syntheticSnapshot);
  snapshot.students.push({ ...snapshot.students[0]!, id: 'synthetic-student-2' });
  const s = new SyntheticApplicationService(snapshot, syntheticLessons);
  const ui: HumanUIPort = bindTrustedSession(s, teacher), agent: AgentSemanticToolsPort = bindTrustedSession(s, teacher);
  const today = ui.invoke(overview);
  expect(today).toMatchObject({ ok: true, sideEffects: [], evidenceIds: ['evidence-1'] });
  if (!today.ok) throw new Error('overview failed');
  const request = { ...detail, params: { ...detail.params, studentId: today.data.rows[0]!.studentId } };
  const selected = agent.invoke(request);
  expect(selected).toMatchObject({ ok: true, data: { studentId: 'synthetic-student-1', lessons: [{ id: 'synthetic-lesson-1' }] }, sideEffects: [] });
  expect(ui.invoke(request)).toMatchObject({ ...selected, replayed: true });
});

test('T02 both read models deny cross-academy access before cached replay', () => {
  const s = service();
  for (const request of [overview, detail]) {
    expect(s.execute(teacher, request).ok).toBe(true);
    expect(s.execute({ ...teacher, academyIds: [] }, request)).toMatchObject({ ok: false, code: 'FORBIDDEN' });
  }
});

test('T03 inactive or reassigned enrollment fails closed for synthetic lesson', () => {
  for (const mode of ['inactive', 'reassigned']) {
    const snapshot = structuredClone(syntheticSnapshot);
    if (mode === 'inactive') snapshot.enrollments[0]!.status = 'inactive';
    else snapshot.enrollments[0]!.studentId = 'other-student';
    expect(new SyntheticApplicationService(snapshot, syntheticLessons).execute(teacher, detail)).toMatchObject({ ok: false, code: 'PRECONDITION_FAILED' });
  }
});

test('T04 textbook edition and video version stay independent', () => {
  expect(service().execute(teacher, detail)).toMatchObject({ ok: true, data: { lessons: [{ material: { textbookEdition: 'synthetic-edition-2', videoVersion: 'synthetic-video-v7', problemRange: { first: 1, last: 5 } } }] } });
});

test('T05 explicit date and timezone required; unknown site timezone preserved', () => {
  const s = service();
  for (const context of [{ ...overview.params.context, date: '2026-02-30' }, { ...overview.params.context, academyTimezone: 'unknown' }, { date: '2026-10-06' }]) {
    expect(s.execute(teacher, { ...overview, params: { context } })).toMatchObject({ ok: false, code: 'INVALID_REQUEST' });
  }
  expect(s.execute(teacher, overview)).toMatchObject({ ok: true, data: { context: { siteTimezone: 'unknown' } } });
  expect(s.execute(teacher, { ...overview, idempotencyKey: 'next-day', params: { context: { ...overview.params.context, date: '2026-10-07' } } })).toMatchObject({ ok: true, data: { rows: [] } });
  expect(s.execute(teacher, { ...overview, idempotencyKey: 'wrong-zone', params: { context: { ...overview.params.context, academyTimezone: 'UTC' } } })).toMatchObject({ ok: false, code: 'PRECONDITION_FAILED' });
});

test('T08 historical score never completes new assignment observations', () => {
  expect(service().execute(teacher, detail)).toMatchObject({ ok: true, data: { lessons: [{ observations: { assignmentConfirmed: 'observed', appScoreObserved: 'not_checked', teacherInspectionObserved: 'not_checked', paperWorkConfirmed: 'unresolved' }, history: [{ assignmentId: 'synthetic-previous-assignment' }] }] } });
});

test('T09 partial observations and absent evidence remain explicit', () => {
  const lessons = structuredClone(syntheticLessons); lessons[0]!.evidenceIds = [];
  expect(new SyntheticApplicationService(syntheticSnapshot, lessons).execute(teacher, detail)).toMatchObject({ ok: true, evidenceIds: [], data: { lessons: [{ observations: { videoSubmitted: 'unresolved', paperWorkConfirmed: 'unresolved' } }] } });
});

test('T10 missing next student does not return previous student detail', () => {
  const s = service(); s.execute(teacher, detail);
  expect(s.execute(teacher, { ...detail, idempotencyKey: 'next-student', params: { ...detail.params, studentId: 'missing' } })).toMatchObject({ ok: false, code: 'NOT_FOUND' });
});

test('T11 teacher decision does not overwrite site observation or grade', () => {
  const s = service();
  expect(s.execute(teacher, { ...overview, action: 'recordTeacherDecision', params: { candidateId: 'candidate-1', decision: 'dismiss', rationale: 'Synthetic review', expectedStatus: 'proposed', expectedDataRevision: 0 } })).toMatchObject({ ok: true });
  expect(s.execute(teacher, detail)).toMatchObject({ ok: true, data: { lessons: [{ observations: { appScoreObserved: 'not_checked', teacherInspectionObserved: 'not_checked', videoSubmitted: 'unresolved' } }] } });
  expect(syntheticSnapshot.gradingEvidence[0]!.outcome).toBe('unmet');
});

test('T14 orphan or cross-scope lesson references fail closed', () => {
  for (const mode of ['student', 'assignment', 'evidence', 'observation']) {
    const lessons = structuredClone(syntheticLessons);
    if (mode === 'student') lessons[0]!.studentId = 'missing';
    if (mode === 'assignment') lessons[0]!.assignmentId = 'missing';
    if (mode === 'evidence') lessons[0]!.evidenceIds = ['missing'];
    if (mode === 'observation') lessons[0]!.observations.assignmentId = 'previous';
    expect(new SyntheticApplicationService(syntheticSnapshot, lessons).execute(teacher, overview)).toMatchObject({ ok: false, code: 'PRECONDITION_FAILED' });
  }
  const snapshot = structuredClone(syntheticSnapshot); snapshot.evidence[0]!.academyId = 'other-academy';
  expect(new SyntheticApplicationService(snapshot, syntheticLessons).execute(teacher, overview)).toMatchObject({ ok: false, code: 'PRECONDITION_FAILED' });
});

test('T16 nested property order replays; changed date with same key conflicts', () => {
  const s = service(); s.execute(teacher, overview);
  expect(s.execute(teacher, { ...overview, params: { context: { siteTimezone: 'unknown', academyTimezone: 'Asia/Seoul', date: '2026-10-06' } } })).toMatchObject({ ok: true, replayed: true });
  expect(s.execute(teacher, { ...overview, params: { context: { ...overview.params.context, date: '2026-10-07' } } })).toMatchObject({ ok: false, code: 'IDEMPOTENCY_CONFLICT' });
});

test('bound principal and returned read models resist caller mutation', () => {
  const principal = structuredClone(teacher), s = service(), ui = bindTrustedSession(s, principal);
  principal.academyIds = [];
  const result = ui.invoke(detail); expect(result.ok).toBe(true);
  if (result.ok) result.data.lessons[0]!.material.videoVersion = 'changed';
  expect(ui.invoke(detail)).toMatchObject({ ok: true, data: { lessons: [{ material: { videoVersion: 'synthetic-video-v7' } }] } });
});
