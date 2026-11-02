import {publicReadFailure,publicFailureFromData} from './read-failure.ts';
/** Never forward arbitrary SDK/upstream text. Only finite public codes and UUIDs survive. */
export class UpstreamToolFailure extends Error {
 readonly failure:ReturnType<typeof publicReadFailure>;
 readonly upstreamOccurrence:string|null;
 constructor(result:any){
  const raw=result?.structuredContent;
  const text=result?.content?.length===1&&result.content[0]?.type==='text'?result.content[0].text:'';
  const code=typeof raw?.failure?.code==='string'?raw.failure.code:typeof raw?.code==='string'?raw.code:typeof raw?.reasonCode==='string'?raw.reasonCode:typeof text==='string'&&text.startsWith('Input validation error:')?'tool_input_invalid':text;
  const failure=raw?.failure?publicFailureFromData(raw.failure):publicReadFailure(Error(typeof code==='string'?code:''),result?'upstream_tool_error':'upstream_result_missing');
  super(failure.code);this.failure=failure;
  const occurrence=result?._meta?.['academy/audit']?.occurrence??raw?.occurrence;
  this.upstreamOccurrence=typeof occurrence==='string'&&/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/.test(occurrence)?occurrence:null;
 }
}
