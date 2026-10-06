import { writeFileSync, mkdirSync, realpathSync, statSync } from 'node:fs';
import { resolve } from 'node:path';
import { SQL } from 'bun';
import { TeacherApplication } from '../src/academy/application';
import { createTeacherServerHandler } from '../src/academy/server';
import { createAuthSecrets, signTeacherSession } from '../src/academy/auth';
import { digest } from '../src/learning/model';
import { PostgresAcademyStore, type StoredStudent } from '../src/academy/postgresStore';
import { teacherPage } from '../src/academy/web/page';

/** Opt-in localhost rehearsal only. Default is in-memory; the observation lane accepts only a scratch test DB. */
const args=process.argv.slice(2);
if(args[0]==='--help'){console.log(JSON.stringify({command:'bun run academy:rehearse --synthetic-only --output-dir <active-profile-scratch-dir>',teacherObservationOption:'--teacher-observation-test-socket',effects:'temporary_loopback_server_and_optional_scratch_test_database_only',data:'invented_students_only',production:false}));process.exit(0);}
const home=process.env.HERMES_HOME;
if(home!=='/opt/data/profiles/dc-gang-a-dev'||![3,5].includes(args.length)||args[0]!=='--synthetic-only'||args[1]!=='--output-dir'||args.length===5&&args[3]!=='--teacher-observation-test-socket')throw Error('explicit_synthetic_rehearsal_required');
const output=resolve(args[2]!),scratch=resolve(home+'/cache/scratch');
if(!output.startsWith(scratch+'/'))throw Error('active_profile_scratch_required');
const testSocket=args.length===5?resolve(args[4]!):null;
if(testSocket&&(!testSocket.startsWith(scratch+'/')||!/^pg-[A-Za-z0-9-]+\/socket$/.test(testSocket.slice(scratch.length+1))||realpathSync(testSocket)!==testSocket||!statSync(testSocket).isDirectory()))throw Error('synthetic_test_socket_required');
mkdirSync(output,{recursive:true,mode:0o700});
const owner=testSocket?'synthetic-browser-'+crypto.randomUUID():'synthetic-browser-owner',actor='synthetic-browser-teacher';
const students:StoredStudent[]=['A','B'].map((label,index)=>({ownerId:owner,id:'fixture-screen-student-'+label,displayName:'Invented Learner '+label,sourceKey:{namespace:'academy_student',literalKey:String(91001+index)},proofRefs:['synthetic_browser_fixture'],registeredAt:'2090-01-01T00:00:00Z'}));
type Current=Awaited<ReturnType<PostgresAcademyStore['current']>>;
function records(student:StoredStudent):Current{
  return ['2090-01-02','2090-01-03'].flatMap(date=>{
    if(date==='2090-01-03'&&student.id.endsWith('B'))return [];
    const rowCount=date==='2090-01-02'&&student.id.endsWith('A')?2:1;
    const dayRecords=Array.from({length:rowCount},(_,index)=>({studentName:student.displayName,date,groupId:index?'22001':'21001',teacherKey:'11001',studentKey:student.sourceKey.literalKey,courseKey:'99001',recordKey:date==='2090-01-03'?'41004':String(student.id.endsWith('B')?41002:41001+index*2),membershipKey:date==='2090-01-03'?'81004':String(student.id.endsWith('B')?81002:81001+index*2),fields:{attendance:student.id.endsWith('B')?'N':'Y',homework:'Invented homework range',lessonMemo:index||date==='2090-01-03'?'Invented makeup label':''}}));
    return dayRecords.map(row=>{
      const acquisition={schemaVersion:1,family:'day_record',status:'observed',records:[],dailyResults:[],dayRecords:[row],requestedScope:{family:'day_record',teacherLabel:'Invented Teacher',date,groupIds:[row.groupId],studentNames:[student.displayName]},observedAt:testSocket?'2025-01-01T00:00:00Z':date+'T00:00:00Z',coverage:'requested_date_groups_students_not_cohort_completeness',unknownStates:['synthetic_not_source_qualification'],nativePrintInvoked:false,automaticRetryAllowed:false};
      const payload={kind:'source_audit' as const,acquisition,contractRef:'synthetic-browser-rehearsal-not-source-qualification'};
      const body={ownerId:owner,studentId:student.id,entityId:'fixture-date-'+date+'-'+row.recordKey,sequence:1,revisionId:'fixture-'+student.id+'-'+date+'-'+row.recordKey,previousRevisionId:null,previousDigest:null,payload,contentDigest:digest(payload),sourceObservedAt:acquisition.observedAt,recordedAt:date+'T00:00:01Z',actorId:actor,idempotencyKey:'fixture-'+date+'-'+row.recordKey,commandDigest:digest({student:student.id,date,recordKey:row.recordKey})};
      return {...body,integrityDigest:digest(body),lastVerifiedAt:date+'T00:00:01Z'};
    });
  });
}
const reader:Pick<PostgresAcademyStore,'students'|'current'|'catalog'|'preparationRevisions'>={
  students:async requested=>requested===owner?structuredClone(students):[],
  current:async(requested,id)=>requested===owner?records(students.find(student=>student.id===id)!):[],
  catalog:async()=>[],
  preparationRevisions:async()=>[],
};
let db:SQL|null=null,store:PostgresAcademyStore|null=null;
if(testSocket){
  db=new SQL({adapter:'postgres',path:testSocket,username:'hermes',database:'dc_academy_test'});
  const [identity]=await db`SELECT current_database() AS database, current_user AS username, inet_server_addr() IS NULL AS local_socket`;
  if(identity?.database!=='dc_academy_test'||identity.username!=='hermes'||identity.local_socket!==true){await db.close();throw Error('synthetic_test_database_required');}
  store=new PostgresAcademyStore(db);await store.migrate();
  for(const student of students){
    const {registeredAt,...registration}=student;await store.registerStudent(registration);
    for(const record of records(student))await store.append({ownerId:owner,studentId:student.id,entityId:record.entityId,actorId:actor,idempotencyKey:record.idempotencyKey,expectedRevisionId:null,sourceObservedAt:record.sourceObservedAt,payload:record.payload});
  }
}
const app=new TeacherApplication(store??reader as PostgresAcademyStore,owner);
await app.classPreparation({ownerId:owner,actorId:actor,role:'teacher'},'2090-01-02');
const build=await Bun.build({entrypoints:[resolve('src/academy/web/client.ts')],target:'browser',format:'esm',minify:true});
if(!build.success)throw Error('rehearsal_client_build_failed');
const script=await build.outputs[0]!.text(),sessionKey=createAuthSecrets().sessionKey;
let handler:(request:Request)=>Promise<Response>,origin='',failure=false,refreshBlocked=false,observationPostCount=0;
const server=Bun.serve({hostname:'127.0.0.1',port:0,async fetch(request){
  const url=new URL(request.url);
  if(url.pathname==='/qa/start'&&request.method==='GET')return new Response(null,{status:303,headers:{Location:'/?date=2090-01-02','Cache-Control':'no-store'}});
  if(url.pathname==='/qa/observation-readback'&&request.method==='GET'){
    if(!store)return Response.json({error:'synthetic_observation_lane_required'},{status:409});
    const histories=[];
    for(const student of students)for(const record of await store.current(owner,student.id))if(record.payload.kind==='teacher_observation')histories.push({studentId:student.id,entityId:record.entityId,records:await store.history(owner,student.id,record.entityId)});
    return Response.json({syntheticOnly:true,observationPostCount,histories,officialSaved:false,parentSent:false},{headers:{'Cache-Control':'no-store'}});
  }
  if(url.pathname==='/qa/control'&&request.method==='POST'){
    if(request.headers.get('Origin')!==origin)return new Response('Origin denied',{status:403});
    const data=await request.json() as {failure?:unknown;refreshBlocked?:unknown};if(Object.keys(data).some(key=>!['failure','refreshBlocked'].includes(key))||typeof data.failure!=='boolean'||(data.refreshBlocked!==undefined&&typeof data.refreshBlocked!=='boolean'))return new Response('Invalid fixture control',{status:400});
    failure=data.failure;if(typeof data.refreshBlocked==='boolean')refreshBlocked=data.refreshBlocked;return Response.json({syntheticOnly:true,failure,refreshBlocked});
  }
  if(failure&&url.pathname==='/api/class')return Response.json({error:'synthetic_source_outage'},{status:503});
  if(url.pathname.startsWith('/api/')){
    if(url.pathname==='/api/teacher-observation'&&request.method==='POST'){
      if(!store)return Response.json({error:'synthetic_observation_lane_required'},{status:409});
      observationPostCount++;
    }
    // Assert ONLY this invented actor inside this isolated fixture server. Never copy browser credentials.
    const headers=new Headers(request.headers);headers.set('Cookie','dc_teacher='+signTeacherSession(sessionKey,owner,actor));
    return handler(new Request(request,{headers}));
  }
  return handler(request);
}});
origin=`http://127.0.0.1:${server.port}`;
handler=createTeacherServerHandler({app,origin,sessionKey,identity:{actorId:actor,username:'synthetic-fixture-only',passwordHash:'no-real-login-credential'},invitationDigest:null,saveIdentity:async()=>{throw Error('rehearsal_write_denied');},refresh:async()=>({status:refreshBlocked?'blocked':'unchanged',reason:refreshBlocked?'synthetic_refresh_blocked':undefined,syntheticOnly:true,sourceWrites:0}),page:teacherPage.replace('<header>','<div class="notice" id="synthetic-rehearsal">SYNTHETIC REHEARSAL · localhost only · no real student data or academy requests</div><header>'),script});
const endpoint={schemaVersion:1,profile:'dc-gang-a-dev',url:origin+'/qa/start',origin,pid:process.pid,syntheticOnly:true,store:store?'synthetic_test_postgres':'minimal_in_memory_read_fixture_not_postgres',teacherObservationEnabled:!!store,realApplicationAndHttpBoundary:true,authentication:'server_owned_invented_actor_not_real_login_acceptance',browserCookiesModified:false,academyRequests:0,productionModified:false};
writeFileSync(output+'/endpoint.json',JSON.stringify(endpoint,null,2)+'\n',{flag:'wx',mode:0o600});
console.log(JSON.stringify(endpoint));
for(const signal of ['SIGINT','SIGTERM'] as const)process.on(signal,async()=>{await server.stop(true);if(db)await db.close();process.exit(0);});
