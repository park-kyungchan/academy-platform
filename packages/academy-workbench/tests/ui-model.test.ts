import { expect, test } from 'bun:test';
import { createUIFixture } from '../fixtures/ui.ts';
import { WorkbenchModel, reviewLabel } from '../src/ui/model.ts';
import { SyntheticApplicationService } from '../src/application/service.ts';
import { bindTrustedSession } from '../src/interfaces/ports.ts';
function setup() {
  const f = createUIFixture();
  return { fixture: f, model: new WorkbenchModel(bindTrustedSession(new SyntheticApplicationService(f.snapshot, f.lessons), { actorId: 'synthetic-teacher', role: 'teacher', academyIds: ['synthetic-academy'] }), f.lessons[0]!.context) };
}
test('UI overview and repeated student switching keep scoped requirement and evidence rows', () => {
  const { model } = setup();
  expect(model.overview()).toMatchObject({ ok: true, data: { rows: [{ studentId: 'synthetic-student-1' }, { studentId: 'synthetic-student-2' }, { studentId: 'synthetic-student-3' }] } });
  for (const n of [1, 1, 2, 3, 1]) {
    expect(model.select(`synthetic-student-${n}`)).toMatchObject({ ok: true, data: { studentId: `synthetic-student-${n}`, videoRequirements: [{ assignmentId: `assignment-${n}` }], problemRequirements: [{ assignmentId: `assignment-${n}` }], evidence: [{ id: `evidence-${n}` }], candidates: [{ id: `candidate-${n}` }] } });
  }
  expect(model.decisions.size).toBe(0);
});
test('UI cancel back and student switch discard pending judgment without write', () => {
  const { model } = setup();
  model.select('synthetic-student-1'); model.preview('candidate-1'); model.cancel();
  expect(model.confirm('accept', 'should not save')).toBeNull();
  model.preview('candidate-1'); model.select(null);
  expect(model.draft).toBeNull();
  model.select('synthetic-student-1'); model.preview('candidate-1'); model.select('synthetic-student-2');
  expect(model.confirm('accept', 'wrong student')).toBeNull();
  expect(model.decisions.size).toBe(0);
});
test('UI explicit judgment requires rationale and repeated submit produces one write', () => {
  const { model, fixture } = setup();
  model.select('synthetic-student-1'); model.preview('candidate-1');
  expect(model.confirm('accept', ' ')).toBeNull();
  expect(model.confirm('accept', 'Synthetic explicit rationale')).toMatchObject({ ok: true });
  expect(model.confirm('accept', 'repeat')).toBeNull();
  expect(model.decisions.size).toBe(1);
  expect(model.select('synthetic-student-1')).toMatchObject({ ok: true, data: { lessons: [{ observations: { teacherInspectionObserved: 'not_checked', videoSubmitted: 'unresolved' } }] } });
  expect(fixture.snapshot.teacherDecisions).toHaveLength(0);
  expect(setup().model.decisions.size).toBe(0);
});
test('UI stale partial and previous score scenarios never advertise completion', () => {
  const { fixture } = setup();
  expect(fixture.lessons.map(reviewLabel)).toEqual(['부분 조회', '오래된 자료', '개별 확인 필요']);
  expect(fixture.lessons.every(l => l.observations.paperWorkConfirmed === 'unresolved')).toBe(true);
  expect(fixture.lessons[2]!.history[0]!.note).toContain('이번 배정의 완료 근거가 아닙니다');
});
