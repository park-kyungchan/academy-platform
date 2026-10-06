import { describe, test, expect } from 'bun:test';
import { SyntheticApplicationService } from '../src/application/service.ts';
import { syntheticSnapshot } from '../fixtures/synthetic.ts';
import { CONTRACT_REVISION } from '../src/domain/canonical.ts';
import { bindTrustedSession } from '../src/interfaces/ports.ts';
import type { ActionRequest, TrustedPrincipal } from '../src/contracts/actions.ts';
const teacher: TrustedPrincipal = { actorId: 'synthetic-teacher', role: 'teacher', academyIds: ['synthetic-academy'] };
const lookup: ActionRequest<'lookupStudent'> = { action: 'lookupStudent', params: { studentId: 'synthetic-student-1' }, academyId: 'synthetic-academy', contractsRevision: CONTRACT_REVISION, idempotencyKey: 'read-1' };
const decision: ActionRequest<'recordTeacherDecision'> = { ...lookup, action: 'recordTeacherDecision', params: { candidateId: 'candidate-1', decision: 'accept', rationale: 'Synthetic review only', expectedStatus: 'proposed', expectedDataRevision: 0 }, idempotencyKey: 'decision-1' };
const service = () => new SyntheticApplicationService(syntheticSnapshot);
describe('permission-checked synthetic foundation', () => {
  test('T06 video and problem requirements resolve independently for decisions', () => {
    for (const requirementId of ['video-1', 'section-1']) {
      const fixture = structuredClone(syntheticSnapshot); fixture.gradingEvidence[0]!.requirementId = requirementId;
      expect(new SyntheticApplicationService(fixture).execute(teacher, decision)).toMatchObject({ ok: true });
    }
  });
  test('T14 decision rejects orphan requirements and evidence belonging to another student', () => {
    for (const mode of ['requirement', 'assignment', 'student']) {
      const fixture = structuredClone(syntheticSnapshot);
      if (mode === 'requirement') fixture.gradingEvidence[0]!.requirementId = 'missing';
      if (mode === 'assignment') fixture.gradingEvidence[0]!.assignmentId = 'missing';
      if (mode === 'student') fixture.enrollments[0]!.studentId = 'other-student';
      expect(new SyntheticApplicationService(fixture).execute(teacher, decision)).toMatchObject({ ok: false, code: 'PRECONDITION_FAILED' });
    }
  });
  test('lookup has no write side effects', () => {
    const s = service(); expect(s.execute(teacher, lookup)).toMatchObject({ ok: true, sideEffects: [] });
    expect(s.execute(teacher, decision).ok).toBe(true);
    expect(syntheticSnapshot.teacherDecisions).toHaveLength(0);
  });
  test('scope denies even administrator', () => expect(service().execute({ ...teacher, role: 'administrator', academyIds: [] }, lookup)).toMatchObject({ ok: false, code: 'FORBIDDEN', sideEffects: [] }));
  test('viewer may read but not record or prepare print', () => {
    const s = service(), viewer = { ...teacher, role: 'viewer' } as const;
    expect(s.execute(viewer, lookup).ok).toBe(true);
    expect(s.execute(viewer, decision)).toMatchObject({ ok: false, code: 'FORBIDDEN' });
    expect(s.execute(viewer, { ...lookup, action: 'preparePrint', params: { studentId: lookup.params.studentId, purpose: 'Synthetic' } })).toMatchObject({ ok: false, code: 'FORBIDDEN' });
  });
  test('revision mismatch fails closed', () => expect(service().execute(teacher, { ...lookup, contractsRevision: 'old' })).toMatchObject({ ok: false, code: 'REVISION_MISMATCH' }));
  test('unknown fields, unknown actions and malformed params rejected', () => {
    for (const invalid of [null, { ...lookup, actorId: 'forged' }, { ...lookup, action: 'print' }, { ...lookup, params: { studentId: '' } }, { ...decision, params: { ...decision.params, decision: 'automatic' } }]) expect(service().execute(teacher, invalid)).toMatchObject({ ok: false, code: 'INVALID_REQUEST' });
  });
  test('ambiguous student cannot be read or printed', () => {
    const fixture = structuredClone(syntheticSnapshot); fixture.students.push({ ...fixture.students[0]! });
    const s = new SyntheticApplicationService(fixture);
    expect(s.execute(teacher, lookup)).toMatchObject({ ok: false, code: 'AMBIGUOUS' });
    expect(s.execute(teacher, { ...lookup, action: 'preparePrint', params: { studentId: lookup.params.studentId, purpose: 'Synthetic' } })).toMatchObject({ ok: false, code: 'AMBIGUOUS' });
  });
  test('missing record does not fall back to first student', () => expect(service().execute(teacher, { ...lookup, params: { studentId: 'missing' } })).toMatchObject({ ok: false, code: 'NOT_FOUND' }));
  test('print preparation never triggers physical printing', () => expect(service().execute(teacher, { ...lookup, action: 'preparePrint', params: { studentId: lookup.params.studentId, purpose: 'Synthetic' } })).toMatchObject({ ok: true, data: { physicalPrintTriggered: false }, sideEffects: [] }));
  test('decision replay is stable, altered payload conflicts, second key fails precondition', () => {
    const s = service(); const first = s.execute(teacher, decision);
    expect(first).toMatchObject({ ok: true, evidenceIds: ['evidence-1'], replayed: false });
    expect(s.execute(teacher, decision)).toMatchObject({ ...first, replayed: true });
    expect(s.execute(teacher, { ...decision, params: { ...decision.params, decision: 'dismiss' } })).toMatchObject({ ok: false, code: 'IDEMPOTENCY_CONFLICT' });
    expect(s.execute(teacher, { ...decision, idempotencyKey: 'new-key' })).toMatchObject({ ok: false, code: 'PRECONDITION_FAILED' });
  });
  test('authorization rechecked before replay', () => {
    const s = service(); s.execute(teacher, decision);
    expect(s.execute({ ...teacher, role: 'viewer' }, decision)).toMatchObject({ ok: false, code: 'FORBIDDEN' });
  });
  test('missing, unknown or cross-scope evidence blocks decisions', () => {
    for (const mode of ['missing','unknown','cross-scope']) {
      const fixture = structuredClone(syntheticSnapshot);
      if (mode === 'missing') fixture.evidence = [];
      if (mode === 'unknown') fixture.gradingEvidence[0]!.outcome = 'unknown';
      if (mode === 'cross-scope') fixture.evidence[0]!.academyId = 'other-academy';
      expect(new SyntheticApplicationService(fixture).execute(teacher, decision)).toMatchObject({ ok: false, code: 'PRECONDITION_FAILED' });
    }
  });
  test('UI and agent ports use identical service boundary', () => {
    const s = service(), ui = bindTrustedSession(s, teacher), agent = bindTrustedSession(s, teacher);
    expect(ui.invoke(lookup)).toMatchObject({ ok: true, replayed: false });
    expect(agent.invoke(lookup)).toMatchObject({ ok: true, replayed: true });
  });
  test('caller mutation cannot corrupt cached results', () => {
    const s = service(), first = s.execute(teacher, lookup);
    if (first.ok) first.data.displayName = 'changed';
    expect(s.execute(teacher, lookup)).toMatchObject({ ok: true, data: { displayName: '합성 학생 A' } });
  });
  test('non-synthetic evidence rejected at reference service boundary', () => {
    const fixture = structuredClone(syntheticSnapshot); fixture.evidence[0]!.source = 'allowed-dom-read';
    expect(() => new SyntheticApplicationService(fixture)).toThrow('Synthetic fixture');
  });
});
