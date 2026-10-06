import type { StudentBookMap, SectionRequirement } from './curriculum.ts';
export interface StudentHistoryEntry {
  id: string; academyId: string; studentId: string; bookId: string; sectionId: string;
  assignedAt: string; dueAt: string | null; requirement: SectionRequirement;
  teacherJudgment: { at: string; teacherId: string; note: string } | null;
}
export interface StudentReadRepository {
  readStudentBooks(academyId: string, studentId: string): readonly StudentBookMap[];
  readStudentHistory(academyId: string, studentId: string): readonly StudentHistoryEntry[];
}
/** No Space API is called here. A future authorized importer supplies this typed snapshot. */
export interface BookFlowSnapshot { contract: 'book-flow-v1'; books: StudentBookMap[]; history: StudentHistoryEntry[]; }
