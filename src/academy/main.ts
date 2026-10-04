import { SQL } from 'bun';
import { chmodSync, existsSync, lstatSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { PostgresAcademyStore } from './postgresStore';
import { TeacherApplication } from './application';
import { createAuthSecrets, secretDigest } from './auth';
import { createTeacherServerHandler, type LoginIdentity } from './server';
import { teacherPage } from './web/page';
import { SourceRefreshCoordinator } from './refresh';
import { nativeBookAcquirer } from './nativeConnector';

interface OwnerRuntime { ownerId:string;hostname:string;postgresSocket:string;database:string;teacherLabel:string;teacherSourceKey:string;collegeSourceKey:string;expectedStudentNames:string[] }
interface ProductAuth { schemaVersion:1;sessionKey:string;invitationDigest:string|null;identity:LoginIdentity|null }
const home=process.env.HERMES_HOME;
if(home!=='/opt/data/profiles/dc-gang-a-dev')throw Error('active_profile_required');
const directory=resolve(home,'academy');
if(lstatSync(directory).isSymbolicLink()||(lstatSync(directory).mode&0o077)!==0)throw Error('private_product_directory_required');
const config=JSON.parse(readFileSync(resolve(directory,'owner.json'),'utf8')) as OwnerRuntime;
const file=resolve(directory,'web-auth.json');
let auth:ProductAuth;
if(!existsSync(file)){
  const generated=createAuthSecrets();
  auth={schemaVersion:1,sessionKey:generated.sessionKey,invitationDigest:secretDigest(generated.invitation),identity:null};
  writeFileSync(file,JSON.stringify(auth)+'\n',{flag:'wx',mode:0o600});
  writeFileSync(resolve(directory,'owner-invitation.url'),'https://'+config.hostname+'/#owner_invite='+generated.invitation,{flag:'wx',mode:0o600});
}else{
  if(lstatSync(file).isSymbolicLink()||(lstatSync(file).mode&0o077)!==0)throw Error('private_auth_file_required');
  auth=JSON.parse(readFileSync(file,'utf8')) as ProductAuth;
}
const db=new SQL({adapter:'postgres',path:config.postgresSocket,username:'hermes',database:config.database,max:5});
const store=new PostgresAcademyStore(db);await store.migrate();
const build=await Bun.build({entrypoints:[resolve(import.meta.dir,'web/client.ts')],target:'browser',format:'esm',minify:true});
if(!build.success)throw Error('teacher_client_build_failed');
const script=await build.outputs[0]!.text();
const app=new TeacherApplication(store,config.ownerId);
const coordinator=new SourceRefreshCoordinator(store,{ownerId:config.ownerId,teacherLabel:config.teacherLabel,teacherKey:config.teacherSourceKey,collegeKey:config.collegeSourceKey},nativeBookAcquirer);
const handler=createTeacherServerHandler({app,origin:'https://'+config.hostname,sessionKey:auth.sessionKey,identity:auth.identity,invitationDigest:auth.invitationDigest,page:teacherPage,script,
  refresh:(context,selection)=>coordinator.refresh(context,selection.studentId),
  consumeInvitation:async()=>{
    auth={...auth,invitationDigest:null};
    const temporary=file+'.new';writeFileSync(temporary,JSON.stringify(auth)+'\n',{flag:'wx',mode:0o600});renameSync(temporary,file);chmodSync(file,0o600);
  },
  saveIdentity:async identity=>{
    auth={...auth,identity,invitationDigest:null};
    const temporary=file+'.new';writeFileSync(temporary,JSON.stringify(auth)+'\n',{flag:'wx',mode:0o600});renameSync(temporary,file);chmodSync(file,0o600);
  }});
const server=Bun.serve({hostname:'127.0.0.1',port:8411,idleTimeout:255,fetch:handler,error:()=>new Response('Operation failed',{status:500,headers:{'Cache-Control':'no-store'}})});
console.log(JSON.stringify({status:'teacher_application_running',listen:'127.0.0.1:'+server.port,origin:'https://'+config.hostname,authentication:'single_teacher_native_vault_setup_required',academyEffect:'read_only',nativeSessionMirror:false}));
for(const signal of ['SIGTERM','SIGINT'])process.on(signal,async()=>{server.stop();await db.close();process.exit(0);});
