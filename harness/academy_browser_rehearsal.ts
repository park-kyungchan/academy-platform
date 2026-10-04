import { writeFileSync, mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { TeacherApplication } from '../src/academy/application';
import { createTeacherServerHandler } from '../src/academy/server';
import { createAuthSecrets, signTeacherSession } from '../src/academy/auth';
import { digest } from '../src/learning/model';
import type { PostgresAcademyStore, StoredStudent } from '../src/academy/postgresStore';
import { teacherPage } from '../src/academy/web/page';

/** Opt-in localhost rehearsal only. No database, academy connector or real credential file is opened. */
const args=process.argv.slice(2);
if(args[0]==='--help'){console.log(JSON.stringify({command:'bun run academy:rehearse --synthetic-only --output-dir <active-profile-scratch-dir>',effects:'temporary_loopback_server_only',data:'invented_in_memory_students',production:false}));process.exit(0);}
const home=process.env.HERMES_HOME;
if(home!=='/opt/data/profiles/dc-gang-a-dev'||args.length!==3||args[0]!=='--synthetic-only'||args[1]!=='--output-dir')throw Error('explicit_synthetic_rehearsal_required');
const output=resolve(args[2]!),scratch=resolve(home+'/cache/scratch');
if(!output.startsWith(scratch+'/'))throw Error('active_profile_scratch_required');
mkdirSync(output,{recursive:true,mode:0o700});
const owner='synthetic-browser-owner',actor='synthetic-browser-teacher';
const students:StoredStudent[]=['A','B'].map((label,index)=>({ownerId:owner,id:'fixture-screen-student-'+label,displayName:'Invented Learner '+label,sourceKey:{namespace:'academy_student',literalKey:String(91001+index)},proofRefs:['synthetic_browser_fixture'],registeredAt:'2090-01-01T00:00:00Z'}));
type Current=Awaited<ReturnType<PostgresAcademyStore['current']>>;
function records(student:StoredStudent):Current{
  return ['2090-01-02','2090-01-03'].flatMap(date=>{
    if(date==='2090-01-03'&&student.id.endsWith('B'))return [];
    const rowCount=date==='2090-01-02'&&student.id.endsWith('A')?2:1;
    const dayRecords=Array.from({length:rowCount},(_,index)=>({studentName:student.displayName,date,groupId:index?'22001':'21001',teacherKey:'11001',studentKey:student.sourceKey.literalKey,courseKey:'99001',recordKey:date==='2090-01-03'?'41004':String(student.id.endsWith('B')?41002:41001+index*2),membershipKey:date==='2090-01-03'?'81004':String(student.id.endsWith('B')?81002:81001+index*2),fields:{attendance:student.id.endsWith('B')?'N':'Y',homework:'Invented homework range',memo:index||date==='2090-01-03'?'Invented makeup label':''}}));
    const acquisition={schemaVersion:1,family:'day_record',status:'observed',records:[],dailyResults:[],dayRecords,requestedScope:{family:'day_record',teacherLabel:'Invented Teacher',date,groupIds:[...new Set(dayRecords.map(row=>row.groupId))],studentNames:[student.displayName]},observedAt:date+'T00:00:00Z',coverage:'requested_date_groups_students_not_cohort_completeness',unknownStates:['synthetic_not_source_qualification'],nativePrintInvoked:false,automaticRetryAllowed:false};
    const payload={kind:'source_audit' as const,acquisition,contractRef:'synthetic-browser-rehearsal-not-source-qualification'};
    const body={ownerId:owner,studentId:student.id,entityId:'fixture-date-'+date,sequence:1,revisionId:'fixture-'+student.id+'-'+date,previousRevisionId:null,previousDigest:null,payload,contentDigest:digest(payload),sourceObservedAt:acquisition.observedAt,recordedAt:date+'T00:00:01Z',actorId:actor,idempotencyKey:'fixture-'+date,commandDigest:digest({student:student.id,date})};
    return [{...body,integrityDigest:digest(body),lastVerifiedAt:date+'T00:00:01Z'}];
  });
}
const reader:Pick<PostgresAcademyStore,'students'|'current'|'catalog'>={
  students:async requested=>requested===owner?structuredClone(students):[],
  current:async(requested,id)=>requested===owner?records(students.find(student=>student.id===id)!):[],
  catalog:async()=>[],
};
const app=new TeacherApplication(reader as PostgresAcademyStore,owner);
const build=await Bun.build({entrypoints:[resolve('src/academy/web/client.ts')],target:'browser',format:'esm',minify:true});
if(!build.success)throw Error('rehearsal_client_build_failed');
const script=await build.outputs[0]!.text(),sessionKey=createAuthSecrets().sessionKey;
let handler:(request:Request)=>Promise<Response>,origin='',failure=false;
const server=Bun.serve({hostname:'127.0.0.1',port:0,async fetch(request){
  const url=new URL(request.url);
  if(url.pathname==='/qa/start'&&request.method==='GET')return new Response(null,{status:303,headers:{Location:'/?date=2090-01-02','Cache-Control':'no-store'}});
  if(url.pathname==='/qa/control'&&request.method==='POST'){
    if(request.headers.get('Origin')!==origin)return new Response('Origin denied',{status:403});
    const data=await request.json() as {failure?:unknown};if(Object.keys(data).length!==1||typeof data.failure!=='boolean')return new Response('Invalid fixture control',{status:400});
    failure=data.failure;return Response.json({syntheticOnly:true,failure});
  }
  if(failure&&url.pathname==='/api/class')return Response.json({error:'synthetic_source_outage'},{status:503});
  if(url.pathname.startsWith('/api/')){
    // Assert ONLY this invented actor inside this isolated fixture server. Never copy browser credentials.
    const headers=new Headers(request.headers);headers.set('Cookie','dc_teacher='+signTeacherSession(sessionKey,owner,actor));
    return handler(new Request(request,{headers}));
  }
  return handler(request);
}});
origin=`http://127.0.0.1:${server.port}`;
handler=createTeacherServerHandler({app,origin,sessionKey,identity:{actorId:actor,username:'synthetic-fixture-only',passwordHash:'no-real-login-credential'},invitationDigest:null,saveIdentity:async()=>{throw Error('rehearsal_write_denied');},page:teacherPage.replace('<header>','<div class="notice" id="synthetic-rehearsal">SYNTHETIC REHEARSAL · localhost only · no real student data or academy requests</div><header>'),script});
const endpoint={schemaVersion:1,profile:'dc-gang-a-dev',url:origin+'/qa/start',origin,pid:process.pid,syntheticOnly:true,store:'minimal_in_memory_read_fixture_not_postgres',realApplicationAndHttpBoundary:true,authentication:'server_owned_invented_actor_not_real_login_acceptance',browserCookiesModified:false,academyRequests:0,productionModified:false};
writeFileSync(output+'/endpoint.json',JSON.stringify(endpoint,null,2)+'\n',{flag:'wx',mode:0o600});
console.log(JSON.stringify(endpoint));
for(const signal of ['SIGINT','SIGTERM'] as const)process.on(signal,()=>{server.stop(true);process.exit(0);});
