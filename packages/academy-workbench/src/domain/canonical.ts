/** Canonical contracts only. IDs are opaque; no upstream endpoint assumptions. */
export const CONTRACT_REVISION = 'foundation-2' as const;
export type Id = string;
export type Role = 'viewer' | 'teacher' | 'administrator';
export interface Scoped { id: Id; academyId: Id; }
export interface Student extends Scoped { displayName: string; }
export interface Enrollment extends Scoped { studentId: Id; classId: Id; status: 'active' | 'inactive'; }
export interface Assignment extends Scoped { enrollmentId: Id; title: string; }
export interface VideoRequirement extends Scoped { assignmentId: Id; videoRef: string; requiredSeconds: number; }
export interface ProblemSectionRequirement extends Scoped { assignmentId: Id; sectionRef: string; requiredProblemCount: number; }
export interface GradingEvidence extends Scoped {
  assignmentId: Id; requirementId: Id; outcome: 'met' | 'unmet' | 'unknown'; sourceEvidenceId: Id;
}
export interface ClinicCandidate extends Scoped {
  studentId: Id; gradingEvidenceIds: Id[]; status: 'proposed';
}
export interface TeacherDecision extends Scoped {
  candidateId: Id; teacherId: Id; decision: 'accept' | 'dismiss'; rationale: string;
}
export interface Evidence extends Scoped {
  source: 'synthetic' | 'allowed-dom-read'; observedAt: string; sourceReference: string;
}
export interface CanonicalSnapshot {
  contractsRevision: typeof CONTRACT_REVISION;
  students: Student[]; enrollments: Enrollment[]; assignments: Assignment[];
  videoRequirements: VideoRequirement[]; problemSectionRequirements: ProblemSectionRequirement[];
  gradingEvidence: GradingEvidence[]; clinicCandidates: ClinicCandidate[];
  teacherDecisions: TeacherDecision[]; evidence: Evidence[];
}
