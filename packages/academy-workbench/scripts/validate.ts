import { syntheticSnapshot as s } from '../fixtures/synthetic.ts';
import { CONTRACT_REVISION } from '../src/domain/canonical.ts';
function check(condition: boolean, label: string): void { if (!condition) throw new Error(label); }
check(s.contractsRevision === CONTRACT_REVISION, 'contract revision');
const tables = [s.students,s.enrollments,s.assignments,s.videoRequirements,s.problemSectionRequirements,s.gradingEvidence,s.clinicCandidates,s.teacherDecisions,s.evidence];
for (const rows of tables) {
  check(new Set(rows.map(r => `${r.academyId}:${r.id}`)).size === rows.length, 'unique scoped IDs');
  check(rows.every(r => r.id.length > 0 && r.academyId === 'synthetic-academy'), 'synthetic scope');
}
const ref = (rows: { id: string; academyId: string }[], id: string, scope: string) => rows.filter(r => r.id === id && r.academyId === scope).length === 1;
for (const e of s.enrollments) check(ref(s.students,e.studentId,e.academyId), 'enrollment student');
for (const a of s.assignments) check(ref(s.enrollments,a.enrollmentId,a.academyId), 'assignment enrollment');
for (const r of [...s.videoRequirements,...s.problemSectionRequirements]) check(ref(s.assignments,r.assignmentId,r.academyId), 'requirement assignment');
for (const r of s.videoRequirements) check(Number.isInteger(r.requiredSeconds) && r.requiredSeconds >= 0, 'video duration');
for (const r of s.problemSectionRequirements) check(Number.isInteger(r.requiredProblemCount) && r.requiredProblemCount >= 0, 'problem count');
for (const g of s.gradingEvidence) {
  check(ref(s.assignments,g.assignmentId,g.academyId), 'grading assignment');
  check(ref([...s.videoRequirements,...s.problemSectionRequirements].filter(r => r.assignmentId === g.assignmentId),g.requirementId,g.academyId), 'grading requirement');
  check(ref(s.evidence,g.sourceEvidenceId,g.academyId), 'grading source');
}
for (const c of s.clinicCandidates) {
  check(ref(s.students,c.studentId,c.academyId), 'candidate student');
  check(c.gradingEvidenceIds.length > 0 && c.gradingEvidenceIds.every(id => ref(s.gradingEvidence,id,c.academyId)), 'candidate evidence');
}
check(s.evidence.every(e => e.source === 'synthetic' && Number.isFinite(Date.parse(e.observedAt))), 'synthetic evidence timestamps');
console.log('PASS: synthetic canonical fixture revision, uniqueness, scope, references and numeric constraints');
