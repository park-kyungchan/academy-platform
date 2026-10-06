import type { BookFlowSnapshot, StudentReadRepository } from '../contracts/student-history.ts';
import { validateBookMap } from '../domain/curriculum.ts';
/** Replace synthetic storage later; typed data stays canonical and Space remains a review projection. */
export class SyntheticStudentReadRepository implements StudentReadRepository {
  private snapshot: BookFlowSnapshot;
  constructor(snapshot: BookFlowSnapshot) {
    if (snapshot.contract !== 'book-flow-v1') throw new Error('Book-flow contract mismatch');
    snapshot.books.forEach(validateBookMap);
    for (const h of snapshot.history) {
      const map = snapshot.books.find(b => b.academyId === h.academyId && b.studentId === h.studentId && b.book.id === h.bookId);
      if (!map?.sections.some(s => s.id === h.sectionId) || h.requirement.sectionId !== h.sectionId) throw new Error('Orphan history');
      if (!map.requirements.some(r => r.id === h.requirement.id && JSON.stringify(r) === JSON.stringify(h.requirement))) throw new Error('History must reference the same canonical requirement/evidence snapshot');
    }
    this.snapshot = structuredClone(snapshot);
  }
  readStudentBooks(academyId: string, studentId: string) { return structuredClone(this.snapshot.books.filter(b => b.academyId === academyId && b.studentId === studentId)); }
  readStudentHistory(academyId: string, studentId: string) { return structuredClone(this.snapshot.history.filter(h => h.academyId === academyId && h.studentId === studentId)); }
}
