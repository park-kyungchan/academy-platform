import type { PageRequest, SourcePage, SourceRow, SyncAuthority, SyncClock, SyncPolicy, SyncRequest, SyncTarget, SyntheticBatchAdapter } from '../src/sync/contracts.ts';
export class VirtualClock implements SyncClock {
  private value=Date.parse('2026-10-06T08:00:00Z');
  now(){return this.value;} advance(ms:number){this.value+=ms;}
  async sleep(ms:number){this.advance(ms);}
}
export class ScenarioAdapter implements SyntheticBatchAdapter {
  readonly mode='synthetic';readonly capabilities={readOnly:true,directHttpVerified:false} as const;
  calls:PageRequest[]=[];
  constructor(private clock:VirtualClock,public respond:(request:PageRequest,call:number)=>SourcePage|Promise<SourcePage>,private readLatencyMs=10){}
  async readPage(request:PageRequest){this.calls.push(structuredClone(request));this.clock.advance(this.readLatencyMs);return this.respond(request,this.calls.length);}
}
export const syncTarget:SyncTarget={academyId:'synthetic-academy',studentId:'synthetic-student-1',sourceStudentId:'synthetic-source-student-1',bookEditionId:'synthetic-book-edition-A'};
export const syncAuthority:SyncAuthority={actorId:'synthetic-teacher',approvedReadBatchId:'synthetic-approved-batch',allowedTargets:[syncTarget],allowedSources:[{id:'synthetic-grading',kind:'student_records'},{id:'synthetic-catalog',kind:'book_catalog'}]};
export const syncRequest:SyncRequest={targets:[syncTarget],sources:[{id:'synthetic-grading',kind:'student_records',timezone:null,catalogRevision:null}],fromDate:'2026-10-01',toDate:'2026-10-06',windowDays:3};
export const syncPolicy:SyncPolicy={maxAttempts:3,baseBackoffMs:10,maxBackoffMs:40,maxPagesPerStep:10,staleAfterMs:60000};
export function sourceRow(request:PageRequest,id='synthetic-row-1',revision=1):SourceRow {
  return {target:structuredClone(request.step.target),sourceId:request.step.source.id,rowId:id,revision,occurredAt:null,effectiveAt:null,values:{sourceState:'unknown',score:null,itemCount:1}};
}
export function sourcePage(request:PageRequest,rows:SourceRow[]=[],patch:Partial<SourcePage>={}):SourcePage {
  return {target:structuredClone(request.step.target),sourceId:request.step.source.id,fromDate:request.step.fromDate,toDate:request.step.toDate,cursor:request.cursor,ordinal:request.ordinal,nextCursor:null,snapshotToken:'synthetic-snapshot-1',exhausted:true,expectedUniqueRows:rows.length,coverage:'complete',observedAt:'2026-10-06T08:00:00Z',sourceTimezone:request.step.source.timezone,rows,...patch};
}
