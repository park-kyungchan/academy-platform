import { syntheticSnapshot } from './synthetic.ts';
import { syntheticLessons } from './lessons.ts';
import type { SyntheticLesson } from '../src/contracts/lesson-views.ts';

export function createUIFixture() {
  const snapshot = structuredClone(syntheticSnapshot);
  snapshot.students = []; snapshot.enrollments = []; snapshot.assignments = [];
  snapshot.videoRequirements = []; snapshot.problemSectionRequirements = [];
  snapshot.gradingEvidence = []; snapshot.clinicCandidates = []; snapshot.evidence = [];
  const lessons: SyntheticLesson[] = [];
  const names = ['가상 학생 별', '가상 학생 달', '가상 학생 해'];
  names.forEach((displayName, i) => {
    const n = i + 1, academyId = 'synthetic-academy', studentId = `synthetic-student-${n}`, assignmentId = `assignment-${n}`;
    snapshot.students.push({ id: studentId, academyId, displayName });
    snapshot.enrollments.push({ id: `enrollment-${n}`, academyId, studentId, classId: 'synthetic-class', status: 'active' });
    snapshot.assignments.push({ id: assignmentId, academyId, enrollmentId: `enrollment-${n}`, title: `가상 수학 과제 ${n}` });
    snapshot.videoRequirements.push({ id: `video-${n}`, academyId, assignmentId, videoRef: `synthetic:video-${n}`, requiredSeconds: 600 + i * 180 });
    snapshot.problemSectionRequirements.push({ id: `section-${n}`, academyId, assignmentId, sectionRef: `synthetic:section-${n}`, requiredProblemCount: 5 + i * 3 });
    snapshot.evidence.push({ id: `evidence-${n}`, academyId, source: 'synthetic', sourceReference: `fixture:ui-${n}`, observedAt: i === 1 ? '2026-10-01T06:00:00Z' : '2026-10-06T05:30:00Z' });
    snapshot.gradingEvidence.push({ id: `grade-${n}`, academyId, assignmentId, requirementId: `section-${n}`, outcome: 'unmet', sourceEvidenceId: `evidence-${n}` });
    snapshot.clinicCandidates.push({ id: `candidate-${n}`, academyId, studentId, gradingEvidenceIds: [`grade-${n}`], status: 'proposed' });
    lessons.push({ ...structuredClone(syntheticLessons[0]!), id: `synthetic-lesson-${n}`, studentId, assignmentId,
      localStartTime: ['16:00', '16:30', '17:00'][i]!,
      material: { textbookEdition: '가상 수학 · 2026 개정판', unit: ['일차방정식', '연립방정식', '함수의 기초'][i]!, section: `${n}-A`, problemRange: { first: 1, last: 5 + i * 3 }, videoVersion: `합성 영상 v${n + 2}` },
      observations: { assignmentId, assignmentConfirmed: 'observed', appScoreObserved: i === 2 ? 'observed' : 'not_checked', videoSubmitted: 'unresolved', teacherInspectionObserved: 'not_checked', paperWorkConfirmed: 'unresolved' },
      evidenceIds: [`evidence-${n}`], questionsToCheck: [i === 1 ? '오래된 조회 자료 다시 확인' : i === 2 ? '배정 이전 점수와 새 과제 구분' : '영상 제출 및 종이 풀이 확인'], preparation: [`가상 ${n}-A 활동지`],
      history: [{ assignmentId: 'synthetic-previous-assignment', note: '2026-10-01 이전 과제 점수 90점 · 이번 배정의 완료 근거가 아닙니다.' }],
      reviewContext: { assignedAt: '2026-10-05T07:00:00Z', observedAt: snapshot.evidence[i]!.observedAt, coverage: i === 0 ? 'partial' : 'all', stale: i === 1,
        reasons: [i === 0 ? '최근 범위만 조회하여 전체 제출 여부를 알 수 없습니다.' : i === 1 ? '조회 시각이 배정 이전입니다. 새 자료 확인이 필요합니다.' : '관찰된 앱 점수는 배정 이전 기록입니다. 새 과제는 미확인입니다.', '영상 제출·교사 검사·종이 풀이는 각각 확인해야 합니다.'] }
    });
  });
  return { snapshot, lessons };
}
