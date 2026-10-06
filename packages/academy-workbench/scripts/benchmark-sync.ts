import { planSync } from '../src/sync/planner.ts';
import { SyntheticSyncEngine } from '../src/sync/engine.ts';
import { MemorySyncStore } from '../src/sync/store.ts';
import { SourceReadError } from '../src/sync/contracts.ts';
import { ScenarioAdapter,VirtualClock,sourcePage,sourceRow,syncAuthority,syncTarget,syncRequest,syncPolicy } from '../fixtures/sync.ts';

export async function benchmarkSync() {
  const targets=[1,2,3].map(n=>({...syncTarget,studentId:`synthetic-student-${n}`,sourceStudentId:`synthetic-source-student-${n}`}));
  const plan=planSync({...syncRequest,targets,sources:[...syncRequest.sources,{id:'synthetic-catalog',kind:'book_catalog',timezone:null,catalogRevision:'synthetic-edition-1'}]},{...syncAuthority,allowedTargets:targets});
  async function scenario(refreshCatalog:boolean) {
    const clock=new VirtualClock(),store=new MemorySyncStore();let transientInjected=false;
    const adapter=new ScenarioAdapter(clock,q=>{
      if(q.step.source.kind==='book_catalog')return sourcePage(q,[sourceRow(q,'synthetic-catalog-definition')]);
      if(!transientInjected&&q.ordinal===1){transientInjected=true;throw new SourceReadError('TEMPORARY',true);}
      const row=sourceRow(q,`${q.step.fromDate}-row-${q.ordinal}`);
      return sourcePage(q,q.ordinal===0?[row,structuredClone(row)]:[row],{expectedUniqueRows:2,nextCursor:q.ordinal===0?'second-page':null,exhausted:q.ordinal===1,coverage:q.ordinal===0?'partial':'complete'});
    });
    const engine=new SyntheticSyncEngine(store,adapter,clock,syncPolicy);
    const cold=await engine.run(plan,{requestId:'synthetic-cold',refreshCatalog});
    const warm=await engine.run(plan,{requestId:'synthetic-warm',refreshCatalog});
    if(cold.state!=='complete'||warm.state!=='complete')throw Error('BENCHMARK_INCOMPLETE');
    return {cold:cold.metrics,warm:warm.metrics,totalQueries:cold.metrics.queries+warm.metrics.queries,totalExtractedRows:cold.metrics.extractedRows+warm.metrics.extractedRows,historyEvents:Object.keys(store.data.history).length};
  }
  const baseline=await scenario(true),optimized=await scenario(false);
  return {synthetic:true,measurement:'Executed synthetic adapters and virtual clock; NOT real academy server performance',scenario:{students:3,studentDateWindows:2,pagesPerWindow:2,sharedBookEditions:1,cycles:2,duplicateRowPerFirstPage:1,transientFailures:1,virtualReadLatencyMs:10},baseline:{description:'Same engine with catalog refresh forced for every target',...baseline},optimized:{description:'Version-scoped catalog cache with unchanged observation no-op handling',...optimized},savedQueries:baseline.totalQueries-optimized.totalQueries,savedExtractedRows:baseline.totalExtractedRows-optimized.totalExtractedRows,limitations:['Not an end-to-end application speed measurement','Virtual latency excludes browser and network costs','Changed source windows are reread; watermarks do not justify skipping mutable historical rows','No live adapter or academy connection exists']};
}
if(import.meta.main){const result=await benchmarkSync();await Bun.write('docs/sync-benchmark-results.json',JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result,null,2));}
