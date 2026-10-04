import type { TeacherApplication, TrustedTeacherContext } from './application';
import { readCookie, secretDigest, signTeacherSession, verifyTeacherSession } from './auth';
import { assertExactKeys } from '../learning/model';
import { validateWebMcpTrialToken } from './originTrial';

export interface LoginIdentity { readonly actorId:string; readonly username:string; readonly passwordHash:string }
export interface TeacherServerOptions {
  readonly app:TeacherApplication; readonly origin:string; readonly sessionKey:string;
  readonly identity:LoginIdentity|null; readonly invitationDigest:string|null;
  readonly saveIdentity:(identity:LoginIdentity)=>Promise<void>;
  readonly consumeInvitation?:()=>Promise<void>;
  readonly page?:string; readonly script?:string;
  readonly originTrialToken?:string;
  readonly refresh?:(context:TrustedTeacherContext,selection:{studentId:string|null})=>Promise<unknown>;
}
const securityHeaders={ 'Cache-Control':'no-store','X-Content-Type-Options':'nosniff','Referrer-Policy':'no-referrer','X-Frame-Options':'DENY',
  'Content-Security-Policy':"default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'" };
function json(value:unknown,status=200,headers:Record<string,string>={}){return new Response(JSON.stringify(value),{status,headers:{...securityHeaders,'Content-Type':'application/json; charset=utf-8',...headers}});}
const cookie=(name:string,value:string,age=3600)=>`${name}=${value}; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=${age}`;
export function createTeacherServerHandler(options:TeacherServerOptions){
  const trial=options.originTrialToken?validateWebMcpTrialToken(options.originTrialToken,options.origin):null;
  const admittedHost=new URL(options.origin).hostname;
  let identity=options.identity,invitationDigest=options.invitationDigest,setupRunning=false;
  const loginAttempts=new Map<string,{count:number;windowStart:number}>();
  const context=(req:Request):TrustedTeacherContext|null=>{
    const raw=readCookie(req.headers.get('Cookie'),'dc_teacher');
    const session=raw?verifyTeacherSession(options.sessionKey,raw):null;
    return session&&identity&&session.ownerId===options.app.ownerId&&session.actorId===identity.actorId?{ownerId:session.ownerId,actorId:session.actorId,role:'teacher'}:null;
  };
  const body=async(req:Request):Promise<Record<string,unknown>>=>{
    if(req.headers.get('Content-Type')?.split(';')[0]!=='application/json')throw Error('json_required');
    if(Number(req.headers.get('Content-Length')??0)>20000)throw Error('input_too_large');
    const text=await req.text();if(text.length>20000)throw Error('input_too_large');
    const data:unknown=JSON.parse(text);if(!data||typeof data!=='object'||Array.isArray(data))throw Error('invalid_input');return data as Record<string,unknown>;
  };
  return async(req:Request):Promise<Response>=>{
    const url=new URL(req.url),path=url.pathname;
    if(!['GET','POST'].includes(req.method))return json({error:'method_not_allowed'},405);
    if(req.method==='POST'&&req.headers.get('Origin')!==options.origin)return json({error:'origin_denied'},403);
    try{
      if(path==='/health'&&req.method==='GET')return json({status:'running',studentDisclosure:false});
      if((path==='/'||path==='/setup')&&req.method==='GET')return new Response(options.page??'Private teacher application',{headers:{...securityHeaders,'Content-Type':'text/html; charset=utf-8',
        ...(trial&&url.hostname===admittedHost&&Date.parse(trial.expiresAt)>Date.now()?{'Origin-Trial':trial.token}:{})}});
      if(path==='/client.js'&&req.method==='GET')return new Response(options.script??'',{headers:{...securityHeaders,'Content-Type':'text/javascript; charset=utf-8'}});
      if(path==='/api/setup-session'&&req.method==='POST'){
        const data=await body(req);assertExactKeys(data,['invitation']);
        if(identity||!invitationDigest||typeof data.invitation!=='string'||secretDigest(data.invitation)!==invitationDigest)return json({error:'setup_not_admitted'},403);
        invitationDigest=null;
        await options.consumeInvitation?.();
        const token=signTeacherSession(options.sessionKey,options.app.ownerId,'bootstrap-owner',undefined,900);
        return json({status:'owner_setup_admitted'},200,{'Set-Cookie':cookie('dc_setup',token,900)});
      }
      if(path==='/api/setup-user'&&req.method==='POST'){
        const token=readCookie(req.headers.get('Cookie'),'dc_setup');const grant=token?verifyTeacherSession(options.sessionKey,token):null;
        if(identity||setupRunning||!grant||grant.ownerId!==options.app.ownerId||grant.actorId!=='bootstrap-owner')return json({error:'setup_not_admitted'},403);
        const data=await body(req);assertExactKeys(data,['username','password']);
        if(typeof data.username!=='string'||!data.username.trim()||data.username.length>200||typeof data.password!=='string'||data.password.length<10||data.password.length>1024)return json({error:'invalid_credentials_shape'},400);
        setupRunning=true;
        try{
          const newIdentity:LoginIdentity={actorId:crypto.randomUUID(),username:data.username,passwordHash:await Bun.password.hash(data.password)};
          await options.saveIdentity(newIdentity);identity=newIdentity;
          return json({status:'teacher_registered'},200,{'Set-Cookie':cookie('dc_teacher',signTeacherSession(options.sessionKey,options.app.ownerId,newIdentity.actorId))});
        }finally{setupRunning=false;}
      }
      if(path==='/api/login'&&req.method==='POST'){
        const now=Date.now(),remote=(req.headers.get('CF-Connecting-IP')??'direct-loopback').slice(0,64);
        const prior=loginAttempts.get(remote);
        const attempt=prior&&now-prior.windowStart<900000?prior:{count:0,windowStart:now};
        if(!prior&&loginAttempts.size>=2048)return json({error:'login_rate_limited'},429);
        attempt.count++;loginAttempts.set(remote,attempt);
        if(attempt.count>5)return json({error:'login_rate_limited'},429,{'Retry-After':'900'});
        const data=await body(req);assertExactKeys(data,['username','password']);
        if(!identity||typeof data.username!=='string'||typeof data.password!=='string'||data.password.length>1024||data.username!==identity.username||!await Bun.password.verify(data.password,identity.passwordHash))return json({error:'login_required'},401);
        return json({status:'teacher_authenticated'},200,{'Set-Cookie':cookie('dc_teacher',signTeacherSession(options.sessionKey,options.app.ownerId,identity.actorId))});
      }
      if(path.startsWith('/api/')){
        const actor=context(req);if(!actor)return json({error:'teacher_authentication_required'},401);
        if(path==='/api/cohort'&&req.method==='GET')return json(await options.app.cohort(actor));
        if(path==='/api/class'&&req.method==='GET'){
          if([...url.searchParams.keys()].some(key=>key!=='date')||url.searchParams.getAll('date').length!==1)return json({error:'unexpected_query_scope'},400);
          const date=url.searchParams.get('date');if(!date)return json({error:'date_required'},400);
          return json(await options.app.classPreparation(actor,date));
        }
        if(path==='/api/student'&&req.method==='GET'){
          if([...url.searchParams.keys()].some(key=>key!=='id'))return json({error:'unexpected_query_scope'},400);
          const id=url.searchParams.get('id');if(!id)return json({error:'student_required'},400);
          return json(await options.app.student(actor,id));
        }
        if(path==='/api/refresh'&&req.method==='POST'){
          const data=await body(req);assertExactKeys(data,['studentId']);
          if(data.studentId!==null&&typeof data.studentId!=='string')return json({error:'invalid_refresh_scope'},400);
          if(typeof data.studentId==='string')await options.app.student(actor,data.studentId);
          if(!options.refresh)return json({status:'blocked',reason:'native_source_connector_unavailable',cacheRetained:true},503);
          return json(await options.refresh(actor,{studentId:data.studentId as string|null}));
        }
        return json({error:'unknown_operation'},404);
      }
      return json({error:'not_found'},404);
    }catch(error){
      const code=error instanceof Error?error.message:'operation_failed';
      const safe=['owner_scope_conflict','student_scope_conflict','invalid_student_selector','invalid_input','json_required','input_too_large','unexpected or missing domain fields'].includes(code)?code:'operation_failed';
      return json({error:safe},safe.includes('scope_conflict')?403:400);
    }
  };
}
