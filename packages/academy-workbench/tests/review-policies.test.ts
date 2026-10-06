import { expect, test } from 'bun:test';
import { paperStatus, mergeManagedMemo, completionSummary } from '../src/domain/review-policies.ts';
import { SyntheticApplicationService } from '../src/application/service.ts';
import { bindTrustedSession } from '../src/interfaces/ports.ts';
import { syntheticSnapshot } from '../fixtures/synthetic.ts';
import { CONTRACT_REVISION } from '../src/domain/canonical.ts';
import type { ActionRequest } from '../src/contracts/actions.ts';

test('T07 explicit paper exception is excused, never confirmed or video completion', () => {
  expect(paperStatus('unresolved', { kind: 'excused', teacherId: 'synthetic-teacher', rationale: 'Synthetic paper exemption' })).toBe('excused');
  expect(paperStatus('not_checked', { kind: 'none' })).toBe('not_checked');
  expect(paperStatus('unresolved', { kind: 'none' })).toBe('unresolved');
  expect(paperStatus('observed', { kind: 'none' })).toBe('confirmed');
});
test('T07 exception without teacher or rationale fails', () => {
  for (const [teacherId, rationale] of [['', 'reason'], ['teacher', ' ']] as const) {
    expect(() => paperStatus('unresolved', { kind: 'excused', teacherId, rationale })).toThrow();
  }
});
test('T12 managed update preserves concurrent human memo verbatim', () => {
  const base = { userText: 'old human note', managedText: 'old generated block', revision: 1 };
  const current = { ...base, userText: 'edited by human\n[managed-looking text]', revision: 2 };
  expect(mergeManagedMemo(base, current, 'new generated block')).toEqual({ ok: true, memo: { ...current, managedText: 'new generated block', revision: 3 } });
  expect(current.managedText).toBe('old generated block');
});
test('T12 changed managed block or regressed revision conflicts without overwrite', () => {
  const base = { userText: 'human', managedText: 'generated', revision: 2 };
  for (const current of [{ ...base, managedText: 'human corrected block', revision: 3 }, { ...base, revision: 1 }]) {
    expect(mergeManagedMemo(base, current, 'replacement')).toEqual({ ok: false, code: 'MANAGED_CONFLICT', memo: current });
  }
});
test('T13 recent range never becomes full denominator even with page end', () => {
  expect(completionSummary({ scope: 'recent', observed: 5, completed: 4, total: 5, pageEndConfirmed: true })).toEqual({ scope: 'recent', observed: 5, completed: 4, denominator: null, percent: null, coverage: 'partial' });
});
test('T13 full denominator requires all scope known total and complete pagination', () => {
  expect(completionSummary({ scope: 'all', observed: 10, completed: 4, total: 10, pageEndConfirmed: true })).toMatchObject({ denominator: 10, percent: 40, coverage: 'complete' });
  for (const c of [{ total: null, pageEndConfirmed: true }, { total: 10, pageEndConfirmed: false }, { total: 20, pageEndConfirmed: true }]) {
    expect(completionSummary({ scope: 'all', observed: 10, completed: 4, ...c })).toMatchObject({ denominator: null, percent: null, coverage: 'partial' });
  }
  expect(completionSummary({ scope: 'all', observed: 0, completed: 0, total: 0, pageEndConfirmed: true }).percent).toBeNull();
  expect(() => completionSummary({ scope: 'all', observed: 2, completed: 3, total: 2, pageEndConfirmed: true })).toThrow();
});

const principal = { actorId: 'synthetic-teacher', role: 'teacher', academyIds: ['synthetic-academy'] } as const;
const preview: ActionRequest<'preparePrint'> = { action: 'preparePrint', academyId: 'synthetic-academy', contractsRevision: CONTRACT_REVISION, idempotencyKey: 'preview', params: { studentId: 'synthetic-student-1', purpose: 'Synthetic decision preview' } };
const decision: ActionRequest<'recordTeacherDecision'> = { ...preview, action: 'recordTeacherDecision', idempotencyKey: 'decision', params: { candidateId: 'candidate-1', decision: 'accept', rationale: 'Synthetic review', expectedStatus: 'proposed', expectedDataRevision: 0 } };
test('T15 changed data revision after preview rejects write then fresh preview succeeds', () => {
  const snapshot = structuredClone(syntheticSnapshot);
  snapshot.clinicCandidates.push({ ...snapshot.clinicCandidates[0]!, id: 'candidate-2' });
  const s = new SyntheticApplicationService(snapshot);
  const ui = bindTrustedSession(s, principal), agent = bindTrustedSession(s, principal);
  const first = ui.invoke(preview);
  expect(first).toMatchObject({ ok: true, data: { dataRevision: 0 } });
  expect(agent.invoke({ ...decision, idempotencyKey: 'other-write', params: { ...decision.params, candidateId: 'candidate-2' } })).toMatchObject({ ok: true });
  expect(ui.invoke(decision)).toMatchObject({ ok: false, code: 'PRECONDITION_FAILED', sideEffects: [] });
  // Replaying an old preview remains old; a fresh key is required to refresh it.
  expect(ui.invoke(preview)).toMatchObject({ ok: true, replayed: true, data: { dataRevision: 0 } });
  const fresh = ui.invoke({ ...preview, idempotencyKey: 'fresh-preview' });
  expect(fresh).toMatchObject({ ok: true, data: { dataRevision: 1 } });
  if (!fresh.ok) throw new Error('preview failed');
  const commit = { ...decision, params: { ...decision.params, expectedDataRevision: fresh.data.dataRevision } };
  expect(ui.invoke(commit)).toMatchObject({ ok: true, replayed: false });
  expect(agent.invoke(commit)).toMatchObject({ ok: true, replayed: true });
});
test('T15 missing malformed revision and viewer write fail closed', () => {
  const s = new SyntheticApplicationService(syntheticSnapshot);
  for (const expectedDataRevision of [undefined, -1, 0.5, '0']) {
    expect(s.execute(principal, { ...decision, params: { ...decision.params, expectedDataRevision } })).toMatchObject({ ok: false, code: 'INVALID_REQUEST' });
  }
  expect(s.execute({ ...principal, role: 'viewer' }, decision)).toMatchObject({ ok: false, code: 'FORBIDDEN' });
});
