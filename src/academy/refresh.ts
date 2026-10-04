import type { TrustedTeacherContext } from './application';
import type { PostgresAcademyStore } from './postgresStore';
import type { SourceDiscoveryRequest } from './nativeBookSource';
import { normalizeCohortDiscovery } from './sourceBridge';
export type Acquisition = {status:'blocked';reason:string}|{status:'observed';data:unknown};
export type SourceAcquirer=(request:SourceDiscoveryRequest)=>Promise<Acquisition>;
export interface SourceRuntime {ownerId:string;teacherLabel:string;teacherKey:string;collegeKey:string}
export class SourceRefreshCoordinator {
  constructor(readonly store:PostgresAcademyStore,readonly config:SourceRuntime,readonly acquire:SourceAcquirer){}
  async refresh(actor:TrustedTeacherContext,studentId:string|null){
    if(actor.ownerId!==this.config.ownerId||actor.role!=='teacher')throw Error('owner_scope_conflict');
    const roster=await this.store.students(this.config.ownerId);
    const selected=studentId===null?roster:roster.filter(student=>student.id===studentId);
    if(selected.length===0)throw Error('student_scope_conflict');
    const request:SourceDiscoveryRequest={teacherLabel:this.config.teacherLabel,teacherKey:this.config.teacherKey,collegeKey:this.config.collegeKey,studentNames:selected.map(student=>student.displayName)};
    const acquisition=await this.acquire(request);
    if(acquisition.status==='blocked')return {status:'blocked' as const,reason:acquisition.reason,cacheRetained:true,academyEffect:'read_only'};
    const normalized=normalizeCohortDiscovery(acquisition.data,request.studentNames);
    if(normalized.ownerId!==this.config.ownerId)throw Error('source_owner_drift');
    for(const student of normalized.students){
      const expected=selected.find(row=>row.displayName===student.registration.displayName);
      if(!expected||expected.id!==student.registration.id||expected.sourceKey.literalKey!==student.registration.sourceKey.literalKey)throw Error('source_student_binding_conflict');
    }
    for(const node of normalized.catalog)await this.store.saveCatalog(normalized.ownerId,node);
    const outcomes=[];
    for(const student of normalized.students){
      const heads=await this.store.current(normalized.ownerId,student.registration.id);
      for(const book of student.books){
        const prior=heads.find(row=>row.entityId===book.entityId);
        outcomes.push(await this.store.append({ownerId:normalized.ownerId,studentId:student.registration.id,entityId:book.entityId,actorId:actor.actorId,
          idempotencyKey:normalized.captureDigest+':'+book.entityId,expectedRevisionId:prior?.revisionId??null,sourceObservedAt:student.observedAt,payload:book.payload}));
      }
    }
    return {status:'observed' as const,studentCount:normalized.students.length,bookCount:outcomes.length,changedVersions:outcomes.filter(row=>row.status==='saved').length,
      unchangedVersions:outcomes.filter(row=>row.status==='unchanged').length,cacheRetained:true,academyEffect:'read_only',officialSaved:false,parentSent:false};
  }
}
