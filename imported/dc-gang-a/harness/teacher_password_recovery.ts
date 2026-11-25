import { closeSync, fsyncSync, lstatSync, openSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { randomUUID } from 'node:crypto';
import { resolve } from 'node:path';
import { TeacherAuthStore } from '../src/academy/teacherAuthStore';

const args=process.argv.slice(2);
if(args.length===0||args[0]==='--help'){
  console.log(JSON.stringify({commands:['bun run academy:password-recovery --prepare','bun run academy:password-recovery --approve-request <exact-public-request-ref> --expires-at <exact-ISO-expiry>'],effect:'Explicit owner-approved recovery for the existing teacher only; does not itself change the password or student history.',browserApproval:'Approve only the exact fresh request observed in the admitted native user Preview. The public request ref is not a credential; the matching signed browser cookie is still required. Never approve a supplied unknown ref by default.',secretDelivery:'Optional link delivery is a private local URL file only; never print or reuse consumed invitations.',recovery:'A newer approval/ticket supersedes the earlier grant; actual password reset atomically consumes its claimed grant and rotates teacher sessions.'}));process.exit(0);
}
const prepare=args.length===1&&args[0]==='--prepare';
const approve=args.length===4&&args[0]==='--approve-request'&&args[2]==='--expires-at';
if(!prepare&&!approve)throw Error('explicit_password_recovery_operation_required');
const home=process.env.HERMES_HOME;
if(home!=='/opt/data/profiles/dc-gang-a-dev')throw Error('active_profile_required');
const directory=resolve(home,'academy'),stat=lstatSync(directory);
if(!stat.isDirectory()||stat.isSymbolicLink()||(stat.mode&0o077)!==0||stat.uid!==process.getuid!())throw Error('private_product_directory_required');
const {hostname}=JSON.parse(readFileSync(resolve(directory,'owner.json'),'utf8')) as {hostname:string};
if(typeof hostname!=='string'||!/^dc\.[a-z0-9.-]+\.[a-z]{2,}$/.test(hostname)||hostname.includes('redacted'))throw Error('owned_teacher_origin_required');
const store=new TeacherAuthStore(resolve(directory,'web-auth.json'));
if(approve){
  const expiry=args[3]!,milliseconds=Date.parse(expiry);
  if(!Number.isFinite(milliseconds)||new Date(milliseconds).toISOString()!==expiry||milliseconds%1000!==0)throw Error('literal_recovery_expiry_required');
  const grant=store.approveBrowserRecovery(args[1]!,milliseconds/1000);
  console.log(JSON.stringify({kind:'teacher_browser_recovery_approved',requestRef:grant.id,expiresAtUtc:expiry,passwordChanged:false,identityChanged:false,studentHistoryChanged:false,academyRequests:0,credentialIncluded:false}));process.exit(0);
}
const grant=store.prepareRecovery();
const url=new URL('/reset','https://'+hostname);url.hash=new URLSearchParams({password_recovery:grant.token}).toString();
const file=resolve(directory,'password-recovery.url'),temporary=file+'.'+randomUUID()+'.new';
const fd=openSync(temporary,'wx',0o600);
try{writeFileSync(fd,url.href+'\n');fsyncSync(fd);}finally{closeSync(fd);}
renameSync(temporary,file);
console.log(JSON.stringify({kind:'teacher_password_recovery_prepared',urlFile:file,grantId:grant.id,expiresAtUtc:new Date(grant.expiresAt*1000).toISOString(),passwordChanged:false,identityChanged:false,studentHistoryChanged:false,academyRequests:0,secretIncluded:false}));
