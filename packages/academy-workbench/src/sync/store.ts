import type { ScopeCoverage, SyncSnapshot } from './contracts.ts';
export class MemorySyncStore {
  private locks=new Set<string>();
  readonly data:SyncSnapshot;
  constructor(snapshot?:SyncSnapshot) { this.data=structuredClone(snapshot??{observations:{},observationVersions:{},checkpoints:{},coverage:{},watermarks:{},catalogCache:{},teacherNotes:{},history:{},attempts:{},contexts:{}}); }
  snapshot():SyncSnapshot { return structuredClone(this.data); }
  /** One store instance owns locks. A durable/distributed adapter needs its own CAS/lease implementation. */
  acquire(keys:string[]):null|(()=>void) {
    const unique=[...new Set(keys)];if(unique.some(k=>this.locks.has(k)))return null;
    unique.forEach(k=>this.locks.add(k));return()=>unique.forEach(k=>this.locks.delete(k));
  }
  setTeacherNote(id:string,note:string) { this.data.teacherNotes[id]=note; }
}
export function coverageIsStale(coverage:ScopeCoverage, now:number, staleAfterMs:number):boolean {
  return coverage.state!=='complete'||coverage.lastSuccessfulAt===null||now-Date.parse(coverage.lastSuccessfulAt)>staleAfterMs;
}
