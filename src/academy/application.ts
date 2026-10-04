import { digest, requireText } from '../learning/model';
import { catalogLevels, type CatalogNode } from './domain';
import type { PostgresAcademyStore } from './postgresStore';
export interface TrustedTeacherContext { readonly ownerId:string; readonly actorId:string; readonly role:'teacher' }
/** The HTTP/native adapter authenticates this context; a caller role string is never its issuer. */
export class TeacherApplication {
  constructor(readonly store:PostgresAcademyStore,readonly ownerId:string){}
  private scope(context:TrustedTeacherContext):void {
    if(context.ownerId!==this.ownerId||context.role!=='teacher'||!context.actorId)throw Error('owner_scope_conflict');
  }
  async cohort(context:TrustedTeacherContext) {
    this.scope(context);
    const students=await this.store.students(this.ownerId);
    return {schemaVersion:1,kind:'teacher_cohort',scope:'registered_teacher_owned_students_not_scheduled_date_completeness',rowCount:students.length,
      students:students.map(student=>({id:student.id,displayName:student.displayName,registeredAt:student.registeredAt})),academyEffect:'read_only',officialSaved:false,parentSent:false};
  }
  async student(context:TrustedTeacherContext,studentId:string) {
    this.scope(context);requireText(studentId,'student id');if(studentId.length>200)throw Error('invalid_student_selector');
    const student=(await this.store.students(this.ownerId)).find(row=>row.id===studentId);
    if(!student)throw Error('student_scope_conflict');
    const [current,catalog]=await Promise.all([this.store.current(this.ownerId,studentId),this.store.catalog(this.ownerId)]);
    const bookSources=current.filter(row=>row.payload.kind==='source_book_registration');
    const rootIds=new Set(bookSources.map(row=>row.payload.kind==='source_book_registration'?row.payload.catalogNodeId:''));
    const nodes=new Map(catalog.map(node=>[node.id,node]));
    const belongs=(node:CatalogNode):boolean=>{
      let currentNode:CatalogNode|undefined=node;
      while(currentNode){if(rootIds.has(currentNode.id))return true;if(currentNode.parent.state==='unknown')return false;currentNode=nodes.get(currentNode.parent.value);}
      return false;
    };
    const selectedCatalog=catalog.filter(belongs);
    return {schemaVersion:1,kind:'student_learning_scene',student:{id:student.id,displayName:student.displayName,sourceKey:student.sourceKey,proofRefs:student.proofRefs},
      sceneRevision:digest({studentId,heads:current.map(row=>({revisionId:row.revisionId,lastVerifiedAt:row.lastVerifiedAt})),catalog:selectedCatalog}),
      books:bookSources.map(row=>({entityId:row.entityId,revisionId:row.revisionId,sourceObservedAt:row.sourceObservedAt,lastVerifiedAt:row.lastVerifiedAt,recordedAt:row.recordedAt,source:row.payload})),
      evidence:current.filter(row=>row.payload.kind!=='source_book_registration'),catalog:selectedCatalog,
      hierarchy:{levels:catalogLevels,unknownLevels:catalogLevels.filter(kind=>!selectedCatalog.some(node=>node.kind===kind))},
      knowledge:{mastery:'not_established',teacherInspection:'unknown',effectiveEnrollment:'unknown',scheduledLessonCompleteness:'unknown',sourceFreshness:'last_successful_acquisition_not_view_time'},
      allowedOperations:['query_student_learning','explain_learning_evidence','refresh_selected_source'],
      limits:['Source registration/learning range is not effective enrollment or book-slot judgment','No missing evidence becomes nonperformance','Printed paper, board and video labels remain separate source namespaces','Current sources do not prove full historical/native-app coverage'],
      officialSaved:false,parentSent:false};
  }
}
