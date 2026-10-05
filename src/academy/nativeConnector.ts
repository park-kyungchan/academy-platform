import { readLiteralKey, readLiteralName } from '../lms/academyReadProtocol';
import type { SourceDiscoveryRequest } from './nativeBookSource';
import type { Acquisition } from './refresh';

/** Actual current consumer capabilities, not source freshness or an effect grant. */
export const academyConnectionStatus=Object.freeze({
  schemaVersion:1,
  retainedQuery:'available',
  acquisition:Object.freeze({transport:'direct_http_api',state:'blocked',reason:'direct_http_authentication_not_qualified',browserFallback:false}),
  officialJournalSave:'not_implemented',
  parentSend:'teacher_owned',
} as const);

/** Retains the exported seam, not the retired automatic native-browser executor. */
export async function nativeBookAcquirer(input:SourceDiscoveryRequest):Promise<Acquisition>{
  if(!Array.isArray(input.studentNames)||input.studentNames.length<1||input.studentNames.length>64||new Set(input.studentNames).size!==input.studentNames.length)throw Error('invalid_source_scope');
  readLiteralName(input.teacherLabel);readLiteralKey(input.teacherKey);readLiteralKey(input.collegeKey);input.studentNames.forEach(readLiteralName);
  if(process.env.HERMES_HOME!=='/opt/data/profiles/dc-gang-a-dev')throw Error('active_profile_required');
  return {status:'blocked',reason:academyConnectionStatus.acquisition.reason};
}
