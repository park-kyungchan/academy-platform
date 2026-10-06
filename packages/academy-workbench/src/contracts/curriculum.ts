/** Legacy synthetic UI projection; canonical r2 types live in book-flow.ts. */
import type { RequirementFacts } from './book-flow.ts';
export type StatusAxes = Pick<RequirementFacts, 'applicability' | 'assignment' | 'requiredness' | 'completion' | 'effort' | 'mapping' | 'coverage'>;
export const GAUSS_SECTIONS = ['개념 이해하기', '개념정복', '필수예제', '유형다지기', '실력쌓기', '개념복습', '4M 듣기와 쓰기', '4M 읽기와 쓰기', '대단원 총괄평가'] as const;
export const MIDDLE_GAUSS_SECTIONS = ['개념 백지 테스트', '개념 예제', '개념 쏙 생각 쑥', '기본 문제', '필수 예제', '유형 다지기', 'Daily Restudy', '대단원 총괄평가'] as const;
export const SECTION_REGISTRY = { elementary: GAUSS_SECTIONS, middle: MIDDLE_GAUSS_SECTIONS, high: [] } as const;
export type SectionName = typeof GAUSS_SECTIONS[number] | typeof MIDDLE_GAUSS_SECTIONS[number];
export interface BookEdition {
  id: string; series: string; grade: number | null; semester: number | null; volume: number | null;
  originalLabel: string; schoolLevel: 'elementary' | 'middle' | 'high'; nativeBookCode: string; version: string;
}
export type BookIdentity = BookEdition;
export interface OutlineNode { id: string; parentId: string | null | 'unknown'; level: 'major_unit' | 'middle_unit' | 'subunit' | 'section' | 'unknown'; title: string; order: number; sectionCoverage?: 'unknown' | 'partial' | 'complete'; }
export type CurriculumUnit = OutlineNode;
export interface CurriculumSection { id: string; scopeUnitId: string; name: SectionName; sourceLabel?: string | null; mappingStatus?: 'unverified' | 'verified'; }
export interface ProgressEvidence {
  id: string; assignmentId: string; kind: 'video_path' | 'problem_grading' | 'teacher_confirmation'; teacherId?: string; observedAt: string;
  outcome: 'complete' | 'missing' | 'partial'; sourceReference: string;
  occurredAt?: string | null; effectiveAt?: string | null;
}
export interface SectionRequirement {
  id: string; sectionId: string; channel: 'video' | 'problem';
  axes: StatusAxes;
  completionAuthority?: 'evidence' | 'teacher';
  assignmentId: string | null; assignedAt: string | null;
  stale: boolean;
  evidence: ProgressEvidence[]; reason: string;
}
export interface StudentBookMap {
  academyId: string; studentId: string; source: 'synthetic';
  book: BookIdentity; units: CurriculumUnit[]; sections: CurriculumSection[]; requirements: SectionRequirement[];
  assignmentInventoryComplete: boolean; observedAt: string; sourceRevision: string;
}
export interface CurriculumReadPort {
  /** Future storage adapter maps a permission-scoped canonical snapshot to this projection. */
  readStudentBooks(academyId: string, studentId: string): readonly StudentBookMap[];
}
