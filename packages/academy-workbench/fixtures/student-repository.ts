import { createCurriculumFixture } from './curriculum.ts';
import { SyntheticStudentReadRepository } from '../src/application/student-read-repository.ts';
import type { StudentHistoryEntry } from '../src/contracts/student-history.ts';
export function createStudentRepository() {
  const books = [1,2,3].flatMap(n => createCurriculumFixture(`synthetic-student-${n}`));
  const history: StudentHistoryEntry[] = books.filter(b=>b.book.id==='gauss-5-2-2').flatMap(b => b.requirements.filter(r=>r.assignmentId !== null).map((r,i)=>({ id: `history-${b.studentId}-${r.id}`, academyId:b.academyId, studentId:b.studentId, bookId:b.book.id, sectionId:r.sectionId, assignedAt:r.assignedAt!, dueAt: i%2===0?'2026-10-05T12:00:00Z':'2026-10-07T12:00:00Z', requirement:structuredClone(r), teacherJudgment: null })));
  return new SyntheticStudentReadRepository({contract:'book-flow-v1',books,history});
}
