import { CONTRACT_REVISION, type CanonicalSnapshot } from '../src/domain/canonical.ts';
export const syntheticSnapshot: CanonicalSnapshot = {
  contractsRevision: CONTRACT_REVISION,
  students: [{ id: 'synthetic-student-1', academyId: 'synthetic-academy', displayName: '합성 학생 A' }],
  enrollments: [{ id: 'enrollment-1', academyId: 'synthetic-academy', studentId: 'synthetic-student-1', classId: 'synthetic-class', status: 'active' }],
  assignments: [{ id: 'assignment-1', academyId: 'synthetic-academy', enrollmentId: 'enrollment-1', title: '합성 과제' }],
  videoRequirements: [{ id: 'video-1', academyId: 'synthetic-academy', assignmentId: 'assignment-1', videoRef: 'synthetic:video', requiredSeconds: 120 }],
  problemSectionRequirements: [{ id: 'section-1', academyId: 'synthetic-academy', assignmentId: 'assignment-1', sectionRef: 'synthetic:section', requiredProblemCount: 5 }],
  gradingEvidence: [{ id: 'grade-1', academyId: 'synthetic-academy', assignmentId: 'assignment-1', requirementId: 'section-1', outcome: 'unmet', sourceEvidenceId: 'evidence-1' }],
  clinicCandidates: [{ id: 'candidate-1', academyId: 'synthetic-academy', studentId: 'synthetic-student-1', gradingEvidenceIds: ['grade-1'], status: 'proposed' }],
  teacherDecisions: [],
  evidence: [{ id: 'evidence-1', academyId: 'synthetic-academy', source: 'synthetic', observedAt: '2026-10-06T00:00:00Z', sourceReference: 'fixture:synthetic-v1' }]
};
