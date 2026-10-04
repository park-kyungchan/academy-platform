import { assertExactKeys, assertKnowledge, assertRefs, canonicalJson, requireText, type Knowledge } from '../learning/model';
import { createAcademySourceAudit } from '../lms/academySourceAudit';
import { validateSourceKey, type SourceKey } from './domain';

/** A current source registration, deliberately not inferred effective enrollment or teacher book-slot assignment. */
export interface SourceBookRegistration {
  readonly kind: 'source_book_registration'; readonly sourceStudent: SourceKey;
  readonly sourceCourse: SourceKey; readonly sourceUserCourse: SourceKey; readonly sourceBook: SourceKey;
  readonly catalogNodeId: string; readonly sourceCourseLabel: string; readonly sourceBookLabel: string;
  readonly sourceStatus: string; readonly sourceProgressLiteral: string; readonly sourceLearningRangeLiteral: string;
  readonly assignmentValidity: Knowledge<string>; readonly role: Knowledge<string>;
  readonly contractRef: string; readonly evidenceRefs: readonly string[];
}
export interface SourceAuditRecord { readonly kind: 'source_audit'; readonly acquisition: Parameters<typeof createAcademySourceAudit>[0]; readonly contractRef: string }
export interface TeacherReportedNote { readonly kind: 'teacher_reported_note'; readonly text: string; readonly directiveRef: string }
export type AcademyRecord = SourceBookRegistration | SourceAuditRecord | TeacherReportedNote;
export function validateAcademyRecord(payload: AcademyRecord): void {
  canonicalJson(payload);
  if (payload.kind === 'source_book_registration') {
    assertExactKeys(payload, ['kind','sourceStudent','sourceCourse','sourceUserCourse','sourceBook','catalogNodeId','sourceCourseLabel','sourceBookLabel','sourceStatus','sourceProgressLiteral','sourceLearningRangeLiteral','assignmentValidity','role','contractRef','evidenceRefs']);
    for (const [key, namespace] of [[payload.sourceStudent,'academy_student'],[payload.sourceCourse,'academy_course'],[payload.sourceUserCourse,'academy_user_course'],[payload.sourceBook,'academy_book']] as const) {
      validateSourceKey(key); if (key.namespace !== namespace) throw Error('source_namespace_conflict');
    }
    for (const text of [payload.catalogNodeId,payload.sourceCourseLabel,payload.sourceBookLabel,payload.sourceStatus,payload.sourceProgressLiteral,payload.sourceLearningRangeLiteral,payload.contractRef]) { requireText(text,'source book field'); if (text.length>16000) throw Error('source_field_too_large'); }
    assertKnowledge<string>(payload.assignmentValidity, value => requireText(value,'assignment validity'));
    assertKnowledge<string>(payload.role, value => requireText(value,'book role'));
    assertRefs(payload.evidenceRefs);
    return;
  }
  if (payload.kind === 'source_audit') {
    assertExactKeys(payload,['kind','acquisition','contractRef']); requireText(payload.contractRef,'source contract');
    createAcademySourceAudit(payload.acquisition); return;
  }
  if (payload.kind === 'teacher_reported_note') {
    assertExactKeys(payload,['kind','text','directiveRef']); requireText(payload.text,'teacher note'); requireText(payload.directiveRef,'attributable directive');
    if (payload.text.length>16000) throw Error('teacher_note_too_large'); return;
  }
  throw Error('unsupported_academy_record');
}
