// Migration-only ORIGINAL-OWNER metadata observer (Root decision 10).
// No constructor, source method, case executor, schema parser or semantic return is invoked.
import {effects,configureTrace} from '../source-adapter-reference-v1/guard.ts';
import {readFileSync,realpathSync,lstatSync} from 'node:fs';
import {resolve,isAbsolute} from 'node:path';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';
const [root,reference,scratch]=process.argv.slice(2);
if(![root,reference,scratch].every(isAbsolute)||realpathSync(scratch)!==scratch)throw Error('metadata_explicit_roots_required');
const stat=lstatSync(scratch);if(stat.uid!==process.getuid!()||(stat.mode&0o077)!==0)throw Error('metadata_private_root_required');
configureTrace(resolve(scratch,'effects.ndjson'));
const pins=JSON.parse(readFileSync(resolve(reference,'SOURCE-PINS.json'),'utf8'));
for(const[p,v]of Object.entries(pins.sources)as any)if(createHash('sha256').update(readFileSync(resolve(root,p))).digest('hex')!==v.sha256)throw Error('metadata_original_source_changed');
const load=(p:string)=>import(pathToFileURL(resolve(root,p)).href);
const {AcademyHttpClient}=await load('apps/academy/dc-gang-a/src/lms/academyHttpClient.ts');
const {NativeDailyRecordBackend,nativeDailyRecordConfigSchema}=await load('modules/academy-workbench/tools/academy-mcp/src/native-dailyrecord-backend.ts');
const {SavedContext}=await load('modules/academy-workbench/tools/academy-mcp/src/saved-context.ts');
const runtimeExports={AcademyHttpClient:Object.getOwnPropertyNames(AcademyHttpClient.prototype).filter(k=>k!=='constructor'),NativeDailyRecordBackend:Object.getOwnPropertyNames(NativeDailyRecordBackend.prototype).filter(k=>k!=='constructor'),SavedContext:Object.getOwnPropertyNames(SavedContext.prototype).filter(k=>k!=='constructor'),nativeConfigSchemaRuntimeAvailable:typeof nativeDailyRecordConfigSchema.parse==='function'};
process.stdout.write(JSON.stringify({owner:'original-javascript-prototype-metadata-only',runtimeExports,effects})+'\n');
