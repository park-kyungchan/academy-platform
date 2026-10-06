import { expect,test } from 'bun:test';
import { hash,planSync } from '../src/sync/planner.ts';
import { SyntheticSyncEngine } from '../src/sync/engine.ts';
import { MemorySyncStore,coverageIsStale } from '../src/sync/store.ts';
import { SourceReadError,type SyntheticBatchAdapter,type SourcePage } from '../src/sync/contracts.ts';
import { VirtualClock,ScenarioAdapter,sourceRow,sourcePage,syncTarget,syncAuthority,syncRequest,syncPolicy } from '../fixtures/sync.ts';
import { benchmarkSync } from '../scripts/benchmark-sync.ts';
const single=()=>planSync({...syncRequest,toDate:'2026-10-01'},syncAuthority);

test('sync planner partitions civil dates deterministically and rejects target or source scope expansion',()=>{
  const plan=planSync(syncRequest,syncAuthority);
  expect(plan.steps.map(s=>[s.fromDate,s.toDate])).toEqual([['2026-10-01','2026-10-03'],['2026-10-04','2026-10-06']]);
  expect(planSync(structuredClone(syncRequest),syncAuthority)).toEqual(plan);
  expect(plan.steps.every(s=>s.source.timezone===null)).toBe(true);
  expect(()=>planSync({...syncRequest,targets:[{...syncTarget,sourceStudentId:'another-student'}]},syncAuthority)).toThrow('TARGET_NOT_ALLOWLISTED');
  expect(()=>planSync({...syncRequest,sources:[{...syncRequest.sources[0]!,id:'other-source'}]},syncAuthority)).toThrow('SOURCE_NOT_ALLOWLISTED');
  expect(()=>planSync({...syncRequest,sources:[{...syncRequest.sources[0]!,kind:'book_catalog'}]},syncAuthority)).toThrow('SOURCE_NOT_ALLOWLISTED');
  expect(()=>planSync({...syncRequest,targets:[{...syncTarget,rawName:'not-admitted'} as typeof syncTarget]},syncAuthority)).toThrow('TARGET_NOT_ALLOWLISTED');
  expect(()=>planSync({...syncRequest,fromDate:'2026-02-30'},syncAuthority)).toThrow('INVALID_DATE_WINDOW');
});
test('sync rejects stale student rows atomically without inferring empty results',async()=>{
  const clock=new VirtualClock(),store=new MemorySyncStore();
  const adapter=new ScenarioAdapter(clock,q=>sourcePage(q,[sourceRow(q),{...sourceRow(q,'other'),target:{...q.step.target,studentId:'previous-student'}}]));
  const result=await new SyntheticSyncEngine(store,adapter,clock,syncPolicy).run(single(),{requestId:'stale'});
  expect(result.state).toBe('partial');expect(result.coverage[0]!.failureCode).toBe('STALE_STUDENT_ROW');expect(result.coverage[0]!.verifiedEmpty).toBe(false);
  expect(Object.keys(store.data.observations)).toHaveLength(0);expect(Object.keys(store.data.history)).toHaveLength(0);
});
test('sync detects page gaps omitted rows and unverified terminal pages',async()=>{
  for(const patch of [{ordinal:2},{expectedUniqueRows:2},{exhausted:null},{expectedUniqueRows:null},{nextCursor:'unexplained-next'}] as Partial<SourcePage>[]) {
    const clock=new VirtualClock(),store=new MemorySyncStore();const adapter=new ScenarioAdapter(clock,q=>sourcePage(q,[sourceRow(q)],patch));
    const result=await new SyntheticSyncEngine(store,adapter,clock,syncPolicy).run(single(),{requestId:'gap'});
    expect(result.state).toBe('partial');expect(result.coverage[0]!.verifiedEmpty).toBe(false);expect(Object.keys(store.data.watermarks)).toHaveLength(0);
  }
});
test('sync empty is a verified query fact only after complete closure, never student inactivity',async()=>{
  const clock=new VirtualClock(),store=new MemorySyncStore(),adapter=new ScenarioAdapter(clock,q=>sourcePage(q));
  const result=await new SyntheticSyncEngine(store,adapter,clock,syncPolicy).run(single(),{requestId:'empty'});
  expect(result.state).toBe('complete');expect(result.coverage[0]!.verifiedEmpty).toBe(true);expect(result.coverage[0]!.timezone).toBeNull();
  expect(Object.keys(store.data.history)).toHaveLength(0);expect(Object.keys(store.data.observations)).toHaveLength(0);
  expect(store.data.attempts[result.actionAttemptId!]!.state).toBe('success_verified');
});
test('sync hashes dedupe and no-op repeated data, revisions preserve history and teacher notes',async()=>{
  const clock=new VirtualClock(),store=new MemorySyncStore();let revision=1,score:number|null=null;
  const adapter=new ScenarioAdapter(clock,q=>{const row=sourceRow(q,'stable-row',revision);row.values.score=score;return sourcePage(q,[row,structuredClone(row)],{expectedUniqueRows:1,observedAt:new Date(clock.now()).toISOString()});});
  const engine=new SyntheticSyncEngine(store,adapter,clock,syncPolicy);store.setTeacherNote('synthetic-note','Teacher wording remains verbatim.');
  const first=await engine.run(single(),{requestId:'first'});expect(first.metrics).toMatchObject({changedRows:1,duplicateRows:1,extractedRows:2});
  const firstEvent=Object.values(store.data.history)[0]!;
  const initialVersions=structuredClone(store.data.observationVersions);clock.advance(10);
  await engine.run(single(),{requestId:'same-revision'});expect(store.data.observationVersions).toEqual(initialVersions);
  clock.advance(1000);revision=2;
  const second=await engine.run(single(),{requestId:'second'});expect(second.metrics.noOpRows).toBe(1);expect(Object.keys(store.data.history)).toHaveLength(1);
  revision=3;score=50;
  const third=await engine.run(single(),{requestId:'third'});expect(third.metrics.changedRows).toBe(1);expect(Object.keys(store.data.history)).toHaveLength(2);
  expect(Object.values(store.data.history).find(e=>e.id!==firstEvent.id)!.supersedesEventIds).toEqual([firstEvent.id]);
  expect(Object.values(store.data.observationVersions).some(v=>v.contentHash===firstEvent.payloadReference.contentHash)).toBe(true);
  expect(store.data.teacherNotes['synthetic-note']).toBe('Teacher wording remains verbatim.');
  revision=1;score=null;const older=await engine.run(single(),{requestId:'older'});expect(older.metrics.olderRows).toBe(1);expect(Object.values(store.data.observations)[0]!.values.score).toBe(50);
});
test('sync checkpoint survives snapshot restore and resumes failed page without rereading committed pages',async()=>{
  const clock=new VirtualClock();let store=new MemorySyncStore();
  const firstAdapter=new ScenarioAdapter(clock,q=>{if(q.ordinal===1)throw new SourceReadError('TEMPORARY',true);return sourcePage(q,[sourceRow(q,'row-a')],{nextCursor:'page-2',exhausted:false,coverage:'partial',expectedUniqueRows:2});});
  const failed=await new SyntheticSyncEngine(store,firstAdapter,clock,{...syncPolicy,maxAttempts:2}).run(single(),{requestId:'resume'});
  expect(failed.metrics).toMatchObject({queries:3,retries:1,changedRows:1});expect(failed.state).toBe('partial');
  store=new MemorySyncStore(store.snapshot());
  const nextAdapter=new ScenarioAdapter(clock,q=>{expect(q.ordinal).toBe(1);expect(q.cursor).toBe('page-2');return sourcePage(q,[sourceRow(q,'row-b')],{expectedUniqueRows:2});});
  const result=await new SyntheticSyncEngine(store,nextAdapter,clock,syncPolicy).run(single(),{requestId:'resume',resume:true});
  expect(result.state).toBe('complete');expect(result.metrics.queries).toBe(1);expect(result.metrics.resumedSteps).toBe(1);expect(result.coverage[0]!.uniqueRows).toBe(2);
  expect(Object.keys(store.data.history)).toHaveLength(2);expect(Object.keys(store.data.watermarks)).toHaveLength(1);
});
test('sync retries only transient reads with bounded exponential backoff and separate latency',async()=>{
  const clock=new VirtualClock(),store=new MemorySyncStore();const adapter=new ScenarioAdapter(clock,(q,n)=>{if(n<3)throw new SourceReadError('TEMPORARY',true);return sourcePage(q);});
  const result=await new SyntheticSyncEngine(store,adapter,clock,syncPolicy).run(single(),{requestId:'retry'});
  expect(result.metrics).toMatchObject({queries:3,retries:2,latencyMs:30,backoffMs:30});
  const blocked=new ScenarioAdapter(clock,()=>{throw new SourceReadError('BLOCKED',true);});
  const denied=await new SyntheticSyncEngine(store,blocked,clock,syncPolicy).run(single(),{requestId:'blocked'});
  expect(denied.metrics.queries).toBe(1);expect(denied.coverage[0]!.state).toBe('blocked');expect(denied.coverage[0]!.lastSuccessfulAt).not.toBeNull();expect(coverageIsStale(denied.coverage[0]!,clock.now(),60000)).toBe(true);
});
test('sync overlapping resource lock excludes a second run and releases after completion',async()=>{
  const clock=new VirtualClock(),store=new MemorySyncStore();let finish:(()=>void)|undefined;
  const pending=new Promise<void>(resolve=>{finish=resolve;});
  const adapter=new ScenarioAdapter(clock,async q=>{await pending;return sourcePage(q);});const engine=new SyntheticSyncEngine(store,adapter,clock,syncPolicy);
  const first=engine.run(single(),{requestId:'one'});
  const second=await engine.run(single(),{requestId:'two'});expect(second.state).toBe('locked');expect(second.actionAttemptId).toBeNull();expect(second.metrics.queries).toBe(0);
  finish!();expect((await first).state).toBe('complete');expect((await engine.run(single(),{requestId:'three'})).state).toBe('complete');
});
test('sync catalog cache shares only exact academy and edition revision and expires explicitly',async()=>{
  const clock=new VirtualClock(),store=new MemorySyncStore();const target2={...syncTarget,studentId:'synthetic-student-2',sourceStudentId:'synthetic-source-student-2'};
  const request={...syncRequest,targets:[syncTarget,target2],sources:[{id:'synthetic-catalog',kind:'book_catalog' as const,timezone:null,catalogRevision:'edition-rev-1'}]};
  const authority={...syncAuthority,allowedTargets:[syncTarget,target2]};
  const adapter=new ScenarioAdapter(clock,q=>sourcePage(q,[sourceRow(q,'catalog-row')]));const engine=new SyntheticSyncEngine(store,adapter,clock,syncPolicy);
  const first=await engine.run(planSync(request,authority),{requestId:'cache-one'});expect(first.metrics).toMatchObject({queries:1,catalogCacheHits:1});
  expect(Object.values(store.data.observations)[0]!.target.studentId).toBeNull();expect(Object.keys(store.data.history)).toHaveLength(0);
  const second=await engine.run(planSync(request,authority),{requestId:'cache-two'});expect(second.metrics.queries).toBe(0);expect(second.metrics.catalogCacheHits).toBe(2);
  request.sources[0]!.catalogRevision='edition-rev-2';expect((await engine.run(planSync(request,authority),{requestId:'cache-new'})).metrics.queries).toBe(1);
  clock.advance(60001);expect((await engine.run(planSync(request,authority),{requestId:'cache-old'})).metrics.queries).toBe(1);
});
test('sync stores minimized typed observations and linked DATA LOGIC ACTION without raw text or invented event time',async()=>{
  const clock=new VirtualClock(),store=new MemorySyncStore();const adapter=new ScenarioAdapter(clock,q=>{const row={...sourceRow(q),rawHtml:'DO_NOT_RETAIN_RAW_BODY',unexpected:'DO_NOT_RETAIN_RAW_BODY'};return {...sourcePage(q,[row]),rawResponse:'DO_NOT_RETAIN_RAW_BODY'};});
  const result=await new SyntheticSyncEngine(store,adapter,clock,syncPolicy).run(single(),{requestId:'minimal'});
  expect(JSON.stringify(store.snapshot())).not.toContain('DO_NOT_RETAIN_RAW_BODY');
  const action=store.data.attempts[result.actionAttemptId!]!,context=store.data.contexts[result.dataContextId!]!,event=store.data.history[action.resultingHistoryEventIds[0]!]!;
  expect(action.dataContextIds).toEqual([context.id]);expect(action.ruleVersions[0]!.ruleId).toBe('exact-allowlist-pagination-revision');expect(context.recordRefs[0]!.entityId).toBe(event.payloadReference.entityId);
  expect(event.kind).toBe('source_observed');expect(event.occurredAt).toBeNull();expect(event.effectiveAt).toBeNull();expect(event.sourceEventTimezone).toBeNull();
});
test('sync refuses non-synthetic adapters and forged plans before calls',async()=>{
  const clock=new VirtualClock(),store=new MemorySyncStore(),adapter=new ScenarioAdapter(clock,q=>sourcePage(q));
  expect(()=>new SyntheticSyncEngine(store,{...adapter,mode:'allowed-dom-read-batch'} as unknown as SyntheticBatchAdapter,clock,syncPolicy)).toThrow('SYNTHETIC_ONLY');
  const plan=single();plan.steps[0]!.target.studentId='forged';await expect(new SyntheticSyncEngine(store,adapter,clock,syncPolicy).run(plan,{requestId:'forged'})).rejects.toThrow('PLAN_CHANGED');expect(adapter.calls).toHaveLength(0);
  const forged=single();forged.steps[0]!.resourceKey='bypass-lock';forged.id=hash(forged.steps);
  await expect(new SyntheticSyncEngine(store,adapter,clock,syncPolicy).run(forged,{requestId:'forged-lock'})).rejects.toThrow('PLAN_IDENTITY_INVALID');
});
test('sync source watermarks preserve partition gaps and failure never becomes verified empty',async()=>{
  const clock=new VirtualClock(),store=new MemorySyncStore();const adapter=new ScenarioAdapter(clock,q=>{if(q.step.fromDate==='2026-10-01')throw Error('private source body must not survive');return sourcePage(q);});
  const result=await new SyntheticSyncEngine(store,adapter,clock,syncPolicy).run(planSync(syncRequest,syncAuthority),{requestId:'gaps'});
  expect(result.state).toBe('partial');expect(result.coverage[0]!.verifiedEmpty).toBe(false);expect(result.coverage[0]!.failureCode).toBe('READ_FAILED');
  expect(Object.values(store.data.watermarks).map(w=>w.fromDate)).toEqual(['2026-10-04']);expect(JSON.stringify(store.snapshot())).not.toContain('private source body');
});
test('sync changed snapshot or conflicting duplicate fails without overwriting a committed page',async()=>{
  for(const issue of ['snapshot','duplicate']) {
    const clock=new VirtualClock(),store=new MemorySyncStore();const adapter=new ScenarioAdapter(clock,q=>{
      if(q.ordinal===0)return sourcePage(q,[sourceRow(q,'same-row')],{exhausted:false,nextCursor:'second',expectedUniqueRows:2});
      const row=sourceRow(q,issue==='duplicate'?'same-row':'other-row');row.values.score=80;
      return sourcePage(q,[row],{expectedUniqueRows:2,snapshotToken:issue==='snapshot'?'changed-snapshot':'synthetic-snapshot-1'});
    });
    const result=await new SyntheticSyncEngine(store,adapter,clock,syncPolicy).run(single(),{requestId:issue});
    expect(result.state).toBe('partial');expect(Object.keys(store.data.observations)).toHaveLength(1);expect(Object.values(store.data.observations)[0]!.values.score).toBeNull();
    expect(result.coverage[0]!.failureCode).toBe(issue==='snapshot'?'SNAPSHOT_CHANGED':'DUPLICATE_CONFLICT');
  }
});
test('sync benchmark reduces measured synthetic reads without duplicate history or invented performance claims',async()=>{
  const result=await benchmarkSync();expect(result.synthetic).toBe(true);expect(result.measurement).toContain('NOT real academy');
  expect(result.optimized.totalQueries).toBeLessThan(result.baseline.totalQueries);expect(result.savedQueries).toBe(5);
  expect(result.optimized.warm.changedRows).toBe(0);expect(result.optimized.historyEvents).toBe(result.baseline.historyEvents);
  expect(result.optimized.cold.retries).toBe(1);expect(result.optimized.warm.catalogCacheHits).toBe(3);
});
test('sync failed catalog refresh invalidates cache and reports stale until a new verified closure',async()=>{
  const clock=new VirtualClock(),store=new MemorySyncStore();let fail=false;
  const adapter=new ScenarioAdapter(clock,q=>{if(fail)throw new SourceReadError('BLOCKED',false);return sourcePage(q,[sourceRow(q)]);});
  const plan=planSync({...syncRequest,sources:[{id:'synthetic-catalog',kind:'book_catalog',timezone:null,catalogRevision:'rev-1'}]},syncAuthority);
  const engine=new SyntheticSyncEngine(store,adapter,clock,syncPolicy);
  expect((await engine.run(plan,{requestId:'catalog-ok'})).coverage[0]!.stale).toBe(false);
  fail=true;const blocked=await engine.run(plan,{requestId:'catalog-failed',refreshCatalog:true});expect(blocked.coverage[0]!.stale).toBe(true);expect(Object.keys(store.data.catalogCache)).toHaveLength(0);
  fail=false;const recovered=await engine.run(plan,{requestId:'catalog-recovered'});expect(recovered.metrics.queries).toBe(1);expect(recovered.coverage[0]!.stale).toBe(false);
});
