import { createHash } from 'node:crypto';
import type { SourceDefinition, SyncAuthority, SyncPlan, SyncRequest, SyncTarget } from './contracts.ts';
const compare=(a:string,b:string)=>a<b?-1:a>b?1:0;
export const stable = (v:unknown):unknown => Array.isArray(v)?v.map(stable):v!==null&&typeof v==='object'?Object.fromEntries(Object.entries(v).sort(([a],[b])=>compare(a,b)).map(([k,x])=>[k,stable(x)])):v;
export const hash = (v:unknown) => createHash('sha256').update(JSON.stringify(stable(v))).digest('hex');
export const sameTarget = (a:SyncTarget,b:SyncTarget) => a.academyId===b.academyId&&a.studentId===b.studentId&&a.sourceStudentId===b.sourceStudentId&&a.bookEditionId===b.bookEditionId;
export const exactTarget = (target:SyncTarget) => Object.keys(target).sort().join(',')==='academyId,bookEditionId,sourceStudentId,studentId'&&Object.values(target).every(v=>typeof v==='string'&&!!v.trim());
export const resourceKeyFor = (target:SyncTarget,source:SourceDefinition) => hash({sourceId:source.id,kind:source.kind,academyId:target.academyId,bookEditionId:target.bookEditionId,...(source.kind==='student_records'?{studentId:target.studentId,sourceStudentId:target.sourceStudentId}:{})});
const validDate = (s:string) => /^\d{4}-\d{2}-\d{2}$/.test(s)&&Number.isFinite(Date.parse(s))&&new Date(s).toISOString().slice(0,10)===s;
/** Civil-date partitioning only. UTC arithmetic here does not infer the source timezone. */
export function planSync(request:SyncRequest, authority:SyncAuthority):SyncPlan {
  if(!authority.actorId.trim()||!authority.approvedReadBatchId.trim()||!request.targets.length||!request.sources.length)throw Error('EMPTY_OR_UNAUTHORIZED_PLAN');
  if(!validDate(request.fromDate)||!validDate(request.toDate)||request.fromDate>request.toDate||!Number.isInteger(request.windowDays)||request.windowDays<1||request.windowDays>366)throw Error('INVALID_DATE_WINDOW');
  const days=(Date.parse(request.toDate)-Date.parse(request.fromDate))/86400000+1;
  if(days>3660||request.targets.length>100||request.sources.length>20)throw Error('PLAN_BOUND_EXCEEDED');
  for(const target of request.targets) if(!exactTarget(target)||!authority.allowedTargets.some(t=>sameTarget(t,target)))throw Error('TARGET_NOT_ALLOWLISTED');
  for(const source of request.sources) if(!authority.allowedSources.some(s=>s.id===source.id&&s.kind===source.kind)||source.timezone!==null&&!source.timezone.trim()||source.catalogRevision!==null&&!source.catalogRevision.trim())throw Error('SOURCE_NOT_ALLOWLISTED');
  if(request.targets.length*request.sources.reduce((n,s)=>n+(s.kind==='book_catalog'?1:Math.ceil(days/request.windowDays)),0)>10000)throw Error('PLAN_BOUND_EXCEEDED');
  const steps:SyncPlan['steps']=[];
  for(const target of request.targets) for(const source of request.sources) {
    const resourceKey=resourceKeyFor(target,source);
    for(let from=Date.parse(request.fromDate);from<=Date.parse(request.toDate);from+=request.windowDays*86400000) {
      const fromDate=new Date(from).toISOString().slice(0,10),toDate=new Date(Math.min(from+(request.windowDays-1)*86400000,Date.parse(request.toDate))).toISOString().slice(0,10);
      const value={resourceKey,target:structuredClone(target),source:structuredClone(source),fromDate,toDate};
      steps.push({id:hash(value),...value});
      if(source.kind==='book_catalog')break; // Version-pinned shared definition, not student history.
    }
  }
  const unique=[...new Map(steps.map(step=>[step.id,step])).values()].sort((a,b)=>compare(a.resourceKey,b.resourceKey)||compare(a.fromDate,b.fromDate)||compare(a.id,b.id));
  return {id:hash(unique),authority:structuredClone(authority),steps:unique};
}
