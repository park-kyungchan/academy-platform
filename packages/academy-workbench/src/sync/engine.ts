import type { CanonicalRef, HistoryEvent } from '../contracts/book-flow.ts';
import { SourceReadError, type Checkpoint, type PageRequest, type SourcePage, type StoredObservation, type SyncClock, type SyncMetrics, type SyncPlan, type SyncPolicy, type SyncResult, type SyncStep, type SyntheticBatchAdapter } from './contracts.ts';
import { exactTarget, hash, resourceKeyFor, sameTarget } from './planner.ts';
import { MemorySyncStore, coverageIsStale } from './store.ts';
import { SYNC_ACTION,SYNC_LOGIC } from './context.ts';

const instant=(v:unknown):v is string=>typeof v==='string'&&/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/.test(v)&&Number.isFinite(Date.parse(v));
const check=(ok:boolean,code:string)=>{if(!ok)throw Error(code);};
const ref=(id:string,kind:string,revision=1,contentHash:string|null=null):CanonicalRef=>({entityId:id,entityKind:kind,schemaVersion:'1.0.0',revision,contentHash});
const emptyMetrics=():SyncMetrics=>({synthetic:true,queries:0,extractedRows:0,retries:0,latencyMs:0,backoffMs:0,changedRows:0,noOpRows:0,duplicateRows:0,olderRows:0,catalogCacheHits:0,resumedSteps:0});
export interface RunOptions { requestId:string; resume?:boolean; refreshCatalog?:boolean; }

/** Only synthetic adapters are executable. The DOM batch interface is declaration-only. */
export class SyntheticSyncEngine {
  constructor(private store:MemorySyncStore,private adapter:SyntheticBatchAdapter,private clock:SyncClock,private policy:SyncPolicy) {
    check(adapter.mode==='synthetic'&&adapter.capabilities.readOnly&&adapter.capabilities.directHttpVerified===false,'SYNTHETIC_ONLY');
    check(Number.isInteger(policy.maxAttempts)&&policy.maxAttempts>=1&&policy.maxAttempts<=5&&Number.isInteger(policy.maxPagesPerStep)&&policy.maxPagesPerStep>=1&&policy.maxPagesPerStep<=1000&&Number.isFinite(policy.baseBackoffMs)&&policy.baseBackoffMs>=0&&policy.maxBackoffMs>=policy.baseBackoffMs&&policy.maxBackoffMs<=60000&&Number.isFinite(policy.staleAfterMs)&&policy.staleAfterMs>=0,'INVALID_POLICY');
  }
  private async fetch(request:PageRequest,metrics:SyncMetrics):Promise<SourcePage> {
    for(let attempt=1;;attempt++) {
      metrics.queries++;
      try { const page=await this.adapter.readPage(structuredClone(request));metrics.extractedRows+=Array.isArray(page.rows)?page.rows.length:0;return page; }
      catch(error) {
        if(!(error instanceof SourceReadError)||!error.retryable||error.code!=='TEMPORARY'||attempt>=this.policy.maxAttempts)throw error;
        metrics.retries++;const delay=Math.min(this.policy.maxBackoffMs,this.policy.baseBackoffMs*2**(attempt-1));metrics.backoffMs+=delay;
        await this.clock.sleep(delay);continue;
      }
    }
  }
  private validatePage(step:SyncStep,cp:Checkpoint,page:SourcePage):void {
    check(!!page&&!!page.target&&sameTarget(step.target,page.target)&&page.sourceId===step.source.id&&page.fromDate===step.fromDate&&page.toDate===step.toDate,'SCOPE_MISMATCH');
    check(page.cursor===cp.cursor&&page.ordinal===cp.ordinal,'PAGE_GAP');
    check(typeof page.snapshotToken==='string'&&page.snapshotToken.length>0&&(cp.snapshotToken===null||cp.snapshotToken===page.snapshotToken),'SNAPSHOT_CHANGED');
    check(page.sourceTimezone===step.source.timezone&&instant(page.observedAt),'SOURCE_TIME_UNVERIFIED');
    check(Array.isArray(page.rows)&&page.rows.length<=10000,'ROW_BOUND');
    if(page.coverage==='blocked')throw new SourceReadError('BLOCKED',false);
    if(page.coverage==='unsupported')throw new SourceReadError('UNSUPPORTED',false);
    check(page.coverage==='complete'||page.coverage==='partial','COVERAGE_UNVERIFIED');
    check(page.expectedUniqueRows===null||Number.isSafeInteger(page.expectedUniqueRows)&&page.expectedUniqueRows>=0,'INVALID_TOTAL');
    check(cp.expectedUniqueRows===null||cp.expectedUniqueRows===page.expectedUniqueRows,'TOTAL_CHANGED');
    check(page.exhausted===true?page.nextCursor===null:page.exhausted===false&&typeof page.nextCursor==='string'&&page.nextCursor.length>0&&!cp.seenCursors.includes(page.nextCursor)&&page.nextCursor!==cp.cursor,'PAGE_END_UNVERIFIED');
  }
  private commitPage(step:SyncStep,checkpoint:Checkpoint,page:SourcePage,metrics:SyncMetrics,attemptId:string):Checkpoint {
    this.validatePage(step,checkpoint,page);
    const cp=structuredClone(checkpoint),pending=new Map<string,StoredObservation>();
    let duplicateRows=0,olderRows=0,noOpRows=0,changedRows=0;
    const history:HistoryEvent[]=[];
    for(const row of page.rows) {
      check(!!row.target&&sameTarget(row.target,step.target)&&row.sourceId===step.source.id,'STALE_STUDENT_ROW');
      check(typeof row.rowId==='string'&&row.rowId.length>0&&row.rowId.length<=200&&Number.isSafeInteger(row.revision)&&row.revision>=1,'INVALID_ROW_IDENTITY');
      check((row.occurredAt===null||instant(row.occurredAt))&&(row.effectiveAt===null||instant(row.effectiveAt)),'INVALID_EVENT_TIME');
      check(!!row.values&&['complete','incomplete','unknown'].includes(row.values.sourceState)&&(row.values.score===null||typeof row.values.score==='number'&&Number.isFinite(row.values.score))&&(row.values.itemCount===null||Number.isSafeInteger(row.values.itemCount)&&row.values.itemCount>=0),'INVALID_VALUES');
      // Copy only selected typed fields. Never retain a DOM body, response object or raw text.
      const values={sourceState:row.values.sourceState,score:row.values.score,itemCount:row.values.itemCount};
      const target=step.source.kind==='book_catalog'?{academyId:step.target.academyId,bookEditionId:step.target.bookEditionId,studentId:null,sourceStudentId:null}:{...step.target};
      const id=hash({resourceKey:step.resourceKey,rowId:row.rowId});
      const contentHash=hash({target,sourceId:step.source.id,rowId:row.rowId,values,occurredAt:row.occurredAt,effectiveAt:row.effectiveAt});
      if(cp.rowHashes[id]) {check(cp.rowHashes[id]===contentHash,'DUPLICATE_CONFLICT');duplicateRows++;continue;}
      cp.rowHashes[id]=contentHash;
      const old=this.store.data.observations[id];
      if(old&&row.revision<old.sourceRevision){olderRows++;continue;}
      check(!old||row.revision!==old.sourceRevision||old.contentHash===contentHash,'REVISION_CONFLICT');
      const observation:StoredObservation={id,sourceId:step.source.id,target,sourceRowId:row.rowId,sourceRevision:row.revision,contentHash,occurredAt:row.occurredAt,effectiveAt:row.effectiveAt,values,lastObservedAt:page.observedAt,historyEventId:old?.historyEventId??null};
      if(old?.contentHash===contentHash){noOpRows++;pending.set(id,observation);continue;}
      changedRows++;
      if(step.source.kind==='student_records') {
        const eventId=hash({observation:id,revision:row.revision,contentHash});observation.historyEventId=eventId;
        history.push({id:eventId,schemaVersion:'1.0.0',revision:row.revision,kind:'source_observed',target:{studentId:step.target.studentId,bookEditionId:step.target.bookEditionId,nodeId:null,assignmentId:null,requirementId:null,classSessionId:null},occurredAt:row.occurredAt,effectiveAt:row.effectiveAt,observedAt:page.observedAt,recordedAt:new Date(this.clock.now()).toISOString(),sourceEventTimezone:page.sourceTimezone,payloadReference:ref(id,'SourceObservation',row.revision,contentHash),provenance:{sourceIds:[step.source.id],coverageIds:[step.id]},supersedesEventIds:old?.historyEventId?[old.historyEventId]:[],correctionReason:old?'Newer source revision; original observation history retained':null,factState:'observed',synthetic:true});
      }
      pending.set(id,observation);
    }
    const uniqueRows=Object.keys(cp.rowHashes).length;
    if(page.expectedUniqueRows!==null)check(uniqueRows<=page.expectedUniqueRows,'ROW_COUNT_EXCEEDED');
    if(page.exhausted)check(page.coverage==='complete'&&page.expectedUniqueRows!==null&&uniqueRows===page.expectedUniqueRows,'INCOMPLETE_CLOSURE');
    // Page transaction: validation of all rows and closure precedes every mutation.
    for(const [id,row]of pending){this.store.data.observations[id]=row;this.store.data.observationVersions[hash({id,revision:row.sourceRevision,contentHash:row.contentHash})]=structuredClone(row);}
    for(const event of history)this.store.data.history[event.id]=event;
    this.store.data.attempts[attemptId]!.resultingHistoryEventIds.push(...history.map(e=>e.id));
    metrics.duplicateRows+=duplicateRows;metrics.olderRows+=olderRows;metrics.noOpRows+=noOpRows;metrics.changedRows+=changedRows;
    cp.snapshotToken=page.snapshotToken;cp.expectedUniqueRows=page.expectedUniqueRows;cp.complete=page.exhausted===true;
    if(cp.cursor!==null)cp.seenCursors.push(cp.cursor);cp.cursor=page.nextCursor;cp.ordinal++;
    this.store.data.checkpoints[cp.key]=cp;
    const previous=this.store.data.coverage[step.id],now=new Date(this.clock.now()).toISOString();
    this.store.data.coverage[step.id]={stepId:step.id,sourceId:step.source.id,target:structuredClone(step.target),fromDate:step.fromDate,toDate:step.toDate,timezone:page.sourceTimezone,state:cp.complete?'complete':'partial',verifiedEmpty:cp.complete&&uniqueRows===0,uniqueRows,checkedAt:now,lastSuccessfulAt:cp.complete?now:previous?.lastSuccessfulAt??null,failureCode:null};
    if(cp.complete) {
      this.store.data.watermarks[step.id]={stepId:step.id,sourceId:step.source.id,resourceKey:step.resourceKey,fromDate:step.fromDate,toDate:step.toDate,completedAt:now,sourceTimezone:page.sourceTimezone,snapshotToken:page.snapshotToken};
      if(step.source.kind==='book_catalog'&&step.source.catalogRevision!==null)this.store.data.catalogCache[step.resourceKey]={resourceKey:step.resourceKey,revision:step.source.catalogRevision,checkedAt:now,sourceTimezone:page.sourceTimezone,recordIds:Object.keys(cp.rowHashes)};
    }
    return cp;
  }
  async run(plan:SyncPlan,options:RunOptions):Promise<SyncResult> {
    check(options.requestId.trim().length>0&&options.requestId.length<=200,'INVALID_REQUEST_ID');
    check(plan.id===hash(plan.steps)&&plan.steps.length>0,'PLAN_CHANGED');
    for(const step of plan.steps){
      check(exactTarget(step.target)&&plan.authority.allowedTargets.some(t=>sameTarget(t,step.target))&&plan.authority.allowedSources.some(s=>s.id===step.source.id&&s.kind===step.source.kind),'PLAN_SCOPE_DENIED');
      check(step.resourceKey===resourceKeyFor(step.target,step.source)&&step.id===hash({resourceKey:step.resourceKey,target:step.target,source:step.source,fromDate:step.fromDate,toDate:step.toDate}),'PLAN_IDENTITY_INVALID');
    }
    const metrics=emptyMetrics(),actionAttemptId=hash({requestId:options.requestId,actor:plan.authority.actorId}),dataContextId=hash({planId:plan.id,requestId:options.requestId});
    const previousAttempt=this.store.data.attempts[actionAttemptId];
    if(previousAttempt)check(options.resume===true&&previousAttempt.dataContextIds[0]===dataContextId&&previousAttempt.authorizationReference===plan.authority.approvedReadBatchId,'REQUEST_REUSE_CONFLICT');
    const release=this.store.acquire(plan.steps.map(s=>s.resourceKey));
    if(!release)return {state:'locked',metrics,coverage:[],actionAttemptId:null,dataContextId:null};
    try {
      const now=new Date(this.clock.now()).toISOString();
      this.store.data.contexts[dataContextId]={id:dataContextId,practicalContextIds:['synthetic-date-first-preparation'],purpose:'Read exact allowlisted source partitions without inferring student completion',recordRefs:[],provenance:{sourceIds:[...new Set(plan.steps.map(s=>s.source.id))],coverageIds:plan.steps.map(s=>s.id)},scope:{studentIds:[...new Set(plan.steps.map(s=>s.target.studentId))],bookEditionIds:[...new Set(plan.steps.map(s=>s.target.bookEditionId))],assignmentIds:[]},asOf:now,coverageIds:plan.steps.map(s=>s.id),limitations:['Synthetic adapter only','Memory store lock is not a cross-process lease','Query events are not student activity']};
      this.store.data.attempts[actionAttemptId]={id:actionAttemptId,actionContractId:SYNC_ACTION.id,contractVersion:SYNC_ACTION.version,requestId:options.requestId,intent:SYNC_ACTION.intent,targetRefs:plan.steps.map(s=>ref(s.id,'QueryPartition')),dataContextIds:[dataContextId],ruleVersions:[{ruleId:SYNC_LOGIC.ruleId,version:SYNC_LOGIC.version}],authorizationReference:plan.authority.approvedReadBatchId,state:'started',performedAt:now,recordedAt:now,verificationEvidenceIds:[],resultingHistoryEventIds:previousAttempt?[...previousAttempt.resultingHistoryEventIds]:[]};
      let partial=false;
      for(const step of plan.steps) {
        const key=hash({requestId:options.requestId,stepId:step.id,actor:plan.authority.actorId});
        let cp=this.store.data.checkpoints[key]??{key,stepId:step.id,cursor:null,ordinal:0,snapshotToken:null,seenCursors:[],rowHashes:{},expectedUniqueRows:null,complete:false};
        if(cp.complete&&options.resume){metrics.resumedSteps++;continue;}
        if(cp.ordinal)metrics.resumedSteps++;
        const cache=this.store.data.catalogCache[step.resourceKey];
        if(step.source.kind==='book_catalog'&&step.source.catalogRevision!==null&&!options.refreshCatalog&&cache?.revision===step.source.catalogRevision&&cache.sourceTimezone===step.source.timezone&&this.clock.now()-Date.parse(cache.checkedAt)<=this.policy.staleAfterMs) {
          metrics.catalogCacheHits++;
          this.store.data.coverage[step.id]={stepId:step.id,sourceId:step.source.id,target:structuredClone(step.target),fromDate:step.fromDate,toDate:step.toDate,timezone:step.source.timezone,state:'complete',verifiedEmpty:cache.recordIds.length===0,uniqueRows:cache.recordIds.length,checkedAt:cache.checkedAt,lastSuccessfulAt:cache.checkedAt,failureCode:null};
          continue;
        }
        try {
          while(!cp.complete) {
            check(cp.ordinal<this.policy.maxPagesPerStep,'PAGE_LIMIT');
            const start=this.clock.now(),backoffBefore=metrics.backoffMs,latencyBefore=metrics.latencyMs;
            let page:SourcePage;
            try {page=await this.fetch({step,cursor:cp.cursor,ordinal:cp.ordinal,snapshotToken:cp.snapshotToken},metrics);}
            finally {metrics.latencyMs=latencyBefore+(this.clock.now()-start)-(metrics.backoffMs-backoffBefore);}
            cp=this.commitPage(step,cp,page,metrics,actionAttemptId);
          }
        } catch(error) {
          partial=true;
          if(step.source.kind==='book_catalog')delete this.store.data.catalogCache[step.resourceKey];
          const safeCodes=['SCOPE_MISMATCH','PAGE_GAP','SNAPSHOT_CHANGED','SOURCE_TIME_UNVERIFIED','ROW_BOUND','COVERAGE_UNVERIFIED','INVALID_TOTAL','TOTAL_CHANGED','PAGE_END_UNVERIFIED','STALE_STUDENT_ROW','INVALID_ROW_IDENTITY','INVALID_EVENT_TIME','INVALID_VALUES','DUPLICATE_CONFLICT','REVISION_CONFLICT','ROW_COUNT_EXCEEDED','INCOMPLETE_CLOSURE','PAGE_LIMIT'];
          const recognized=error instanceof SourceReadError&&['TEMPORARY','BLOCKED','UNSUPPORTED'].includes(error.code)?error.code:error instanceof Error&&safeCodes.includes(error.message)?error.message:'READ_FAILED';
          const prior=this.store.data.coverage[step.id];
          this.store.data.coverage[step.id]={stepId:step.id,sourceId:step.source.id,target:structuredClone(step.target),fromDate:step.fromDate,toDate:step.toDate,timezone:step.source.timezone,state:recognized==='BLOCKED'?'blocked':recognized==='UNSUPPORTED'?'unsupported':'partial',verifiedEmpty:false,uniqueRows:Object.keys(cp.rowHashes).length,checkedAt:new Date(this.clock.now()).toISOString(),lastSuccessfulAt:prior?.lastSuccessfulAt??null,failureCode:recognized};
          this.store.data.checkpoints[key]=cp;
        }
      }
      partial ||= plan.steps.some(s=>this.store.data.coverage[s.id]?.state!=='complete');
      const action=this.store.data.attempts[actionAttemptId]!;action.state=partial?'unknown':'success_verified';action.verificationEvidenceIds=plan.steps.filter(s=>this.store.data.coverage[s.id]?.state==='complete').map(s=>s.id);
      this.store.data.contexts[dataContextId]!.recordRefs=Object.values(this.store.data.observations).filter(r=>plan.steps.some(s=>r.sourceId===s.source.id&&r.target.academyId===s.target.academyId&&r.target.bookEditionId===s.target.bookEditionId&&(r.target.studentId===null||r.target.studentId===s.target.studentId))).map(r=>ref(r.id,'SourceObservation',r.sourceRevision,r.contentHash));
      return {state:partial?'partial':'complete',metrics,coverage:structuredClone(plan.steps.map(s=>{const coverage=this.store.data.coverage[s.id]!;return {...coverage,stale:coverageIsStale(coverage,this.clock.now(),this.policy.staleAfterMs)};})),actionAttemptId,dataContextId};
    } finally {release();}
  }
}
