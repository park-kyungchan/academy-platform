import type { ActionAttempt, DataContext, HistoryEvent } from '../contracts/book-flow.ts';

export interface SyncTarget { academyId:string; studentId:string; sourceStudentId:string; bookEditionId:string; }
export interface SourceDefinition { id:string; kind:'student_records'|'book_catalog'; timezone:string|null; catalogRevision:string|null; }
/** Trusted boundary input, never inferred from a requested student or a returned DOM row. */
export interface SyncAuthority { actorId:string; approvedReadBatchId:string; allowedTargets:readonly SyncTarget[]; allowedSources:readonly Pick<SourceDefinition,'id'|'kind'>[]; }
export interface SyncRequest { targets:SyncTarget[]; sources:SourceDefinition[]; fromDate:string; toDate:string; windowDays:number; }
export interface SyncStep { id:string; resourceKey:string; target:SyncTarget; source:SourceDefinition; fromDate:string; toDate:string; }
export interface SyncPlan { id:string; authority:SyncAuthority; steps:SyncStep[]; }
export interface SourceRow {
  target:SyncTarget; sourceId:string; rowId:string; revision:number;
  /** Source effective/event times are independent from the time of extraction. */
  occurredAt:string|null; effectiveAt:string|null;
  values:{ sourceState:'complete'|'incomplete'|'unknown'; score:number|null; itemCount:number|null };
}
export interface PageRequest { step:SyncStep; cursor:string|null; ordinal:number; snapshotToken:string|null; }
export interface SourcePage {
  target:SyncTarget; sourceId:string; fromDate:string; toDate:string;
  cursor:string|null; ordinal:number; nextCursor:string|null;
  snapshotToken:string; exhausted:boolean|null; expectedUniqueRows:number|null;
  coverage:'complete'|'partial'|'blocked'|'unsupported'; observedAt:string; sourceTimezone:string|null;
  rows:SourceRow[];
}
export interface ReadBatchAdapter {
  readonly mode:'synthetic'|'allowed-dom-read-batch';
  readonly capabilities:{readonly readOnly:true; readonly directHttpVerified:false};
  readPage(request:PageRequest):Promise<SourcePage>;
}
/** Interface only: no DOM controller, credentials, endpoint or network implementation. */
export interface AllowedDomBatchAdapter extends ReadBatchAdapter { readonly mode:'allowed-dom-read-batch'; }
export interface SyntheticBatchAdapter extends ReadBatchAdapter { readonly mode:'synthetic'; }
export interface SyncClock { now():number; sleep(ms:number):Promise<void>; }
export interface SyncPolicy { maxAttempts:number; baseBackoffMs:number; maxBackoffMs:number; maxPagesPerStep:number; staleAfterMs:number; }
export interface SyncMetrics { synthetic:true; queries:number; extractedRows:number; retries:number; latencyMs:number; backoffMs:number; changedRows:number; noOpRows:number; duplicateRows:number; olderRows:number; catalogCacheHits:number; resumedSteps:number; }
export interface StoredObservation {
  id:string; sourceId:string; target:Omit<SyncTarget,'studentId'|'sourceStudentId'>&{studentId:string|null;sourceStudentId:string|null};
  sourceRowId:string; sourceRevision:number; contentHash:string; occurredAt:string|null; effectiveAt:string|null;
  values:SourceRow['values']; lastObservedAt:string; historyEventId:string|null;
}
export interface Checkpoint {
  key:string; stepId:string; cursor:string|null; ordinal:number; snapshotToken:string|null;
  seenCursors:string[]; rowHashes:Record<string,string>; expectedUniqueRows:number|null; complete:boolean;
}
export interface ScopeCoverage {
  stepId:string; sourceId:string; target:SyncTarget; fromDate:string; toDate:string; timezone:string|null;
  state:'not_queried'|'partial'|'complete'|'blocked'|'unsupported'; verifiedEmpty:boolean;
  uniqueRows:number; checkedAt:string; lastSuccessfulAt:string|null; failureCode:string|null;
}
export interface Watermark { stepId:string; sourceId:string; resourceKey:string; fromDate:string; toDate:string; completedAt:string; sourceTimezone:string|null; snapshotToken:string; }
export interface CatalogCache { resourceKey:string; revision:string; checkedAt:string; sourceTimezone:string|null; recordIds:string[]; }
export interface SyncSnapshot {
  observations:Record<string,StoredObservation>; observationVersions:Record<string,StoredObservation>; checkpoints:Record<string,Checkpoint>; coverage:Record<string,ScopeCoverage>;
  watermarks:Record<string,Watermark>; catalogCache:Record<string,CatalogCache>;
  teacherNotes:Record<string,string>; history:Record<string,HistoryEvent>; attempts:Record<string,ActionAttempt>; contexts:Record<string,DataContext>;
}
export interface SyncResult { state:'complete'|'partial'|'locked'; metrics:SyncMetrics; coverage:Array<ScopeCoverage&{stale:boolean}>; actionAttemptId:string|null; dataContextId:string|null; }
export class SourceReadError extends Error { constructor(readonly code:'TEMPORARY'|'BLOCKED'|'UNSUPPORTED', readonly retryable:boolean) { super(code); } }
