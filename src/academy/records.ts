import { assertExactKeys, assertInstant, assertKnowledge, assertRefs, canonicalJson, requireText, type Knowledge } from '../learning/model';
import { createAcademySourceAudit } from '../lms/academySourceAudit';
import { validateSourceKey, type SourceKey } from './domain';
import {validateCourseInventory,validatePrestudySnapshot,validatePreparationScope,type SourceCourseInventory,type SourcePrestudySnapshot,type TeacherBookPreparationScope} from './bookPreparation';
import {requirementsForSeries,type CycleActivity} from './classroomCyclePolicy';

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
export interface ObservationOccurrence { date:string; groupId:string; courseKey:string; recordKey:string; membershipKey:string }
/** An attributable original/correction, never an inspection approval, official save or delivery receipt. */
export interface TeacherObservation {
  kind:'teacher_observation';sourceStudent:SourceKey;occurrence:ObservationOccurrence;
  basisRevisionId:string;reportedAt:string;text:string;
}
/** Trusted intake attributes the report; this label is not authentication or an actual performance timestamp. */
export interface TeacherActivityRevision {
  kind:'teacher_activity_revision';sourceStudent:SourceKey;sourceCourse:SourceKey;sourceBook:SourceKey;sourceMinorUnit:SourceKey;
  activity:CycleActivity;scope:'minor_unit';decision:'accepted'|'reopen';effectiveDate:string;
  authority:'explicit_teacher_report';directiveRef:string;evidenceRefs:readonly string[];
}
export interface TeacherPreparationAssignment extends Omit<TeacherBookPreparationScope,'kind'> {
  kind:'teacher_preparation_assignment';sourceStudent:SourceKey;effectiveDate:string;
}
export type AcademyRecord = SourceBookRegistration | SourceAuditRecord | TeacherReportedNote | TeacherObservation | SourceCourseInventory | SourcePrestudySnapshot | TeacherActivityRevision | TeacherPreparationAssignment;
export function validateAcademyRecord(payload: AcademyRecord): void {
  canonicalJson(payload);
  if(payload.kind==='teacher_observation'){
    assertExactKeys(payload,['kind','sourceStudent','occurrence','basisRevisionId','reportedAt','text']);
    validateSourceKey(payload.sourceStudent);if(payload.sourceStudent.namespace!=='academy_student')throw Error('source_namespace_conflict');
    assertExactKeys(payload.occurrence,['date','groupId','courseKey','recordKey','membershipKey']);
    const date=payload.occurrence.date;
    if(typeof date!=='string'||!/^\d{4}-\d{2}-\d{2}$/.test(date)||!Number.isFinite(Date.parse(date+'T00:00:00Z'))||new Date(date+'T00:00:00Z').toISOString().slice(0,10)!==date)throw Error('invalid_observation_date');
    for(const field of ['groupId','courseKey','recordKey','membershipKey'] as const){const key=payload.occurrence[field];if(typeof key!=='string'||!/^[1-9][0-9]*$/.test(key)||!Number.isSafeInteger(Number(key)))throw Error('invalid_observation_occurrence');}
    requireText(payload.basisRevisionId,'evidence basis');requireText(payload.text,'teacher observation');assertInstant(payload.reportedAt);
    if(payload.basisRevisionId.length>200||payload.text.length>16000)throw Error('observation_too_large');return;
  }
  if(payload.kind==='source_course_inventory'){validateCourseInventory(payload);return;}
  if(payload.kind==='source_prestudy_snapshot'){validatePrestudySnapshot(payload);return;}
  if(payload.kind==='teacher_preparation_assignment'){
    validateSourceKey(payload.sourceStudent);if(payload.sourceStudent.namespace!=='academy_student')throw Error('source_namespace_conflict');
    const {kind,sourceStudent,effectiveDate,...scope}=payload;
    validatePreparationScope({kind:'teacher_book_preparation_scope',...scope});
    if(!/^\d{4}-\d{2}-\d{2}$/.test(effectiveDate)||!Number.isFinite(Date.parse(effectiveDate+'T00:00:00Z'))||new Date(effectiveDate+'T00:00:00Z').toISOString().slice(0,10)!==effectiveDate)throw Error('invalid_preparation_date');return;
  }
  if(payload.kind==='teacher_activity_revision'){
    assertExactKeys(payload,['kind','sourceStudent','sourceCourse','sourceBook','sourceMinorUnit','activity','scope','decision','effectiveDate','authority','directiveRef','evidenceRefs']);
    for(const [key,namespace] of [[payload.sourceStudent,'academy_student'],[payload.sourceCourse,'academy_course'],[payload.sourceBook,'academy_book'],[payload.sourceMinorUnit,'academy_minor_unit']] as const){validateSourceKey(key);if(key.namespace!==namespace)throw Error('source_namespace_conflict');}
    if(payload.scope!=='minor_unit'||payload.authority!=='explicit_teacher_report'||!['accepted','reopen'].includes(payload.decision)||!requirementsForSeries('gauss').some(r=>r.activity===payload.activity&&r.completionAuthority==='explicit_teacher_judgment'))throw Error('teacher_activity_authority');
    const date=payload.effectiveDate;
    if(!/^\d{4}-\d{2}-\d{2}$/.test(date)||!Number.isFinite(Date.parse(date+'T00:00:00Z'))||new Date(date+'T00:00:00Z').toISOString().slice(0,10)!==date)throw Error('invalid_preparation_date');
    requireText(payload.directiveRef,'teacher directive');assertRefs(payload.evidenceRefs);return;
  }
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
