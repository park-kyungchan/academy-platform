import {existsSync,readFileSync,readdirSync,statSync,writeFileSync,mkdirSync,appendFileSync,realpathSync} from 'node:fs';
import {resolve,relative} from 'node:path';
import {createHash} from 'node:crypto';
import {attribution,scope,target,time,fields,json,put,event,initialize,prepare,service,observation,context,proposal} from './fixture.ts';
import {applyJournalHttpAction} from '../../apps/academy/dc-gang-a/src/lms/lessonJournalHttp.ts';
import {DailyRecordWorkflowStore,executionKey,workflowLockKey,ledgerGroup,requestBoundApproval,studentExecutions,unresolvedOnField,fieldExecutionState} from '../../modules/academy-workbench/tools/academy-mcp/src/dailyrecord-workflow.ts';
import {canonicalActionDigest,createDailyRecordSaveAdmission,liveSaveRequestEvidence} from '../../modules/academy-workbench/tools/academy-mcp/src/dailyrecord-save-admission.ts';
import {durableSaveConsumeOnce} from '../../modules/academy-workbench/tools/academy-mcp/src/native-dailyrecord-backend.ts';
import {registerCismWeeklyTools} from '../../modules/academy-workbench/tools/academy-mcp/src/cism-weekly-tools.ts';
import {cismHash} from '../../apps/academy/dc-gang-a/src/lms/cismWeeklyProtocol.ts';
const [op,root,arg='s10-request']=process.argv.slice(2);
if(!root||!process.env.S10_EVIDENCE_ROOT||!resolve(root).startsWith(realpathSync(process.env.S10_EVIDENCE_ROOT)+'/'))throw Error('s10_owned_evidence_root_required');
initialize(root);const hash=(s:string)=>createHash('sha256').update(s).digest('hex');
const events=()=>existsSync(resolve(root,'events.jsonl'))?readFileSync(resolve(root,'events.jsonl'),'utf8').trim().split('\n').filter(Boolean).map(x=>JSON.parse(x)):[];
function files(path:string,prefix=''):any[]{if(!existsSync(path))return [];return readdirSync(path).sort().flatMap(name=>{const p=resolve(path,name),key=prefix+name;return statSync(p).isDirectory()?files(p,key+'/'):[{path:key,bytes:readFileSync(p).toString('base64')}];});}
function snapshot(){return {files:files(resolve(root,'store')),source:json(resolve(root,'source.json')),events:events(),downstream:{pdf:events().filter(e=>e.operation==='pdf').length,print:events().filter(e=>e.operation==='print').length,parentSend:events().filter(e=>e.operation==='parent_send').length}};}
const output=(result:unknown)=>console.log(JSON.stringify({op,result,snapshot:snapshot()}));
if(op==='prepare'){const a=JSON.parse(arg);output(await prepare(root,a.requestRef,a.field,a.value,a.date));}
else if(op==='mode'){const s=json(resolve(root,'source.json'));s.mode=arg;put(resolve(root,'source.json'),s);output({mode:arg});}
else if(op==='edit-source'){const s=json(resolve(root,'source.json'));Object.assign(s,JSON.parse(arg));put(resolve(root,'source.json'),s);output(s);}
else if(op==='save'||op==='crash-save'){
 const {svc,workflow}=service(root);const input=json(resolve(root,(op==='save'?arg:'s10-request')+'.input.json'));
 if(op==='crash-save'&&arg==='index-only'){const old=workflow.store.addToLedger.bind(workflow.store);workflow.store.addToLedger=(...args:any[])=>{old(args[0],args[1]);process.exit(70);};}
 if(op==='crash-save'&&arg==='durable-fence'){const old=workflow.store.publishOnce.bind(workflow.store);workflow.store.publishOnce=((kind:any,id:any,record:any)=>{const result=old(kind,id,record);if(kind==='execution'&&result.created)process.exit(71);return result;}) as never;}
 output(await svc.save(input));
}
else if(op==='edited-save'){
 const {svc}=service(root),input=json(resolve(root,'s10-request.input.json')),change=JSON.parse(arg);Object.assign(input,change);output(await svc.save(input));
}
else if(op==='reconcile'){
 const input=json(resolve(root,'s10-request.input.json')),options=arg.startsWith('{')?JSON.parse(arg):{};
 const {svc}=service(root,options.clock??Date.parse(time));output(await svc.reconcile({context:input.context,attribution,...(options.resolution?{resolution:options.resolution}:{})}));
}
else if(op==='status'){
 const {svc,workflow}=service(root),s=arg.startsWith('{')?JSON.parse(arg):scope;
 output({public:await svc.executionStatus({scope:s,requestRef:'s10-request',attribution}),executions:studentExecutions(workflow.store,s),samePhysical:unresolvedOnField(workflow.store,s,'progress','13'),otherField:unresolvedOnField(workflow.store,s,'homework','13'),otherRecord:unresolvedOnField(workflow.store,s,'progress','99')});
}
else if(op==='seed-previous'){
 const {workflow}=service(root);const action={attemptId:'s10-prior',target,field:'previousHomeworkAssessment',effectRecordKey:'12',before:'3',after:'4',wireDigest:'a'.repeat(64)};
 const a=requestBoundApproval(workflow.store,{requestRef:'s10-request',attribution:attribution as never,scope,reviewDigest:'c'.repeat(64),reviewed:[{field:'previousHomeworkAssessment',effectRecordKey:'12',before:'3',after:'4'}]});output(await a.consumeApproval(action));
}
else if(op==='state-matrix'){
 const {workflow}=service(root),store=workflow.store,states:any[]=[];
 for(const [name,kind,record] of [['index-only',null,null],['fenced','execution',{}],['persisted','outcome',{status:'persisted'}],['noop','outcome',{status:'noop'}],['not-persisted','outcome',{status:'not_persisted'}],['unresolved','outcome',{status:'unresolved'}],['conflict','outcome',{status:'conflict'}],['observed','reconciliation',{classification:'persisted',basis:'readback_new_value'}],['resolved-old','reconciliation',{classification:'not_persisted',basis:'user_instruction'}],['resolved-conflict','reconciliation',{classification:'conflict',basis:'readback_after_returned_transport'}]] as const){
  const action={attemptId:'matrix-'+name,target,field:'progress',effectRecordKey:'13',before:fields.progress,after:'- 다음 진도',wireDigest:'a'.repeat(64)},key=executionKey('matrix-'+name,scope,'progress','13',action.before,action.after);
  if(name==='index-only')store.addToLedger(ledgerGroup(scope),key);
  else {await requestBoundApproval(store,{requestRef:'matrix-'+name,attribution:attribution as never,scope,reviewDigest:'d'.repeat(64),reviewed:[{field:'progress',effectRecordKey:'13',before:action.before,after:action.after}]}).consumeApproval(action);if(kind!=='execution')store.publishOnce(kind as never,key,record);}
  states.push({name,key,state:fieldExecutionState(store,key)});
 }
 output({states,executions:studentExecutions(store,scope),ledger:store.ledger(ledgerGroup(scope))});
}
else if(op==='approval'){
 const action={attemptId:'s10-admission',target,field:'progress',effectRecordKey:'13',before:fields.progress,after:'- 지정 문제 풀이 진행.',wireDigest:'a'.repeat(64)},current={runtime:'s10-synthetic-owner',requestRef:'s10-user-request',requestTextSha256:hash('Synthetic explicit current user instruction.'),currentUserEventRef:'s10:event:1'};
 const admission={schemaVersion:'academy.dailyrecord-save-admission/1' as const,...current,admitted:[{studentId:scope.studentId,lessonDate:scope.lessonDate,field:'progress' as const,effectRecordKey:'13',after:action.after}]};
 const path=resolve(root,'current-user-event.json');put(path,current);const owner={readCurrentRequest:liveSaveRequestEvidence(path),consumeOnce:durableSaveConsumeOnce(resolve(root,'consume'))};
 const a=createDailyRecordSaveAdmission(admission,owner),foreign=await a.consumeApproval({...action,target:{...target,studentKey:'77002'}}),foreignRecord=await a.consumeApproval({...action,effectRecordKey:'99'});
 put(path,{...current,currentUserEventRef:'s10:event:foreign'});const stale=await a.consumeApproval(action);put(path,current);
 const first=await a.consumeApproval(action),duplicate=await a.consumeApproval(action),freshObject=await createDailyRecordSaveAdmission(admission,owner).consumeApproval(action);
 output({current,admission,action,admissionDigest:a.admissionDigest,foreign,foreignRecord,stale,first,duplicate,freshObject,consumed:files(resolve(root,'consume')),expectedActionDigest:canonicalActionDigest(action)});
}
else if(op==='lock'){
 const options=JSON.parse(arg),store=new DailyRecordWorkflowStore(resolve(root,'store')),key=workflowLockKey(scope),path=resolve(root,'store/locks',hash(key)+'.flock');
 if(options.started)writeFileSync(resolve(root,options.started),String(process.pid));
 const start=performance.now();try{await store.withLock(key,async()=>{const s=statSync(path);event(root,{operation:'workflow_acquired',dev:s.dev,ino:s.ino});writeFileSync(resolve(root,options.marker??'held'),'held');if(options.hold)await new Promise(r=>setTimeout(r,options.hold));},{waitMs:options.waitMs??180,pollMs:10});output({status:'acquired',elapsedMs:performance.now()-start,lock:statSync(path)});}catch(e){output({status:'refused',code:(e as Error).message,elapsedMs:performance.now()-start,lock:statSync(path)});}
}
else if(op==='cism'){
 const dir=resolve(root,'cism');mkdirSync(dir,{recursive:true,mode:0o700});const field={fieldRef:'1'.repeat(64),kind:'narrative',label:'Invented S10 narrative',value:'',studentName:null,groupName:null,source:{fieldName:'s_2'}};
 const report=()=>{const s=json(resolve(root,'source.json'));return {date:'2040-10-15',periodStart:'2040-10-14',periodEnd:'2040-10-20',weekLabel:'Invented week',teacherLabel:'Invented S10 Teacher',teacherKey:scope.teacherId,wireSha256:'2'.repeat(64),reportSha256:cismHash([s.cismValue??'']),observedAt:time,fields:[{...field,value:s.cismValue??''}],sourceMode:'live_source',pdfGenerated:false};};
 const registry=new Map<string,any>();const backend={readCismReport:async()=>report(),applyCismField:async(a:any,authorize:any)=>{if(!await authorize())throw Error('cism_review_required');event(root,{operation:'fake_cism_dispatch',fieldRef:a.fieldRef});if(json(resolve(root,'source.json')).cismFail)throw Error('s10_fake_cism_timeout');const s=json(resolve(root,'source.json'));s.cismValue=a.after;put(resolve(root,'source.json'),s);return {status:'persisted' as const,fieldRef:a.fieldRef,report:report() as never};}};
 registerCismWeeklyTools({registerTool:(n:string,c:any,h:any)=>registry.set(n,{c,h})} as never,backend,dir);
 const call=async(n:string,v:any)=>{const t=registry.get(n);return await t.h(t.c.inputSchema.parse(v));};
 const key=cismHash([scope.teacherId,report().periodStart,field.fieldRef]);
 if(arg==='seed-legacy') {const legacy={fieldRef:field.fieldRef,before:'',after:'Legacy supported S10 narrative.',requestRef:'s10-legacy',reviewDigest:'e'.repeat(64)};writeFileSync(resolve(dir,key+'.dispatch.json'),JSON.stringify(legacy),{mode:0o600});output({legacy,key,files:files(dir)});}
 else if(arg==='get'){output({response:await call('academy_cism_weekly_get',{date:report().date}),files:files(dir)});}
 else {const before=report().fields[0].value,input={date:report().date,expectedReportSha256:report().reportSha256,request:{requestRef:arg,instruction:'Save only this exact supported fictional CISM narrative.'},changes:[{fieldRef:field.fieldRef,before,after:before?before+' Additional supported S10 guidance.':'Supported S10 narrative.',edit:before?'complete_existing_narrative':'fill_empty',evidenceRefs:['s10:synthetic:dated-evidence']}],attribution};const review=await call('academy_cism_weekly_review',input);const digest=review.structuredContent?.reviewDigest;output({input,review,response:digest?await call('academy_cism_weekly_save',{...input,reviewDigest:digest}):null,files:files(dir)});}
}
else if(op==='snapshot')output({});
else if(op==='review-held'){
 const {svc,workflow,state}=service(root),c=context(),handle='dailyrecord-observation:s10-unknown';
 const s=state();delete s.fields.progress;put(resolve(root,'source.json'),s);
 workflow.store.publishOnce('observation',handle,{observation:observation(s),scope,requestRef:c.currentRequestRef,subject:{displayName:target.studentName},createdBy:attribution,createdAt:time});
 output(await svc.review({context:c,observationHandle:handle,proposals:[proposal(c,'- 지정 문제 풀이 진행.')],attribution}));
}
else if(op==='binding-refusals'){
 const {workflow,backend}=service(root),store=workflow.store,action={attemptId:'s10-binding',target,field:'progress',effectRecordKey:'13',before:fields.progress,after:'- 지정 문제 풀이 진행.',wireDigest:'a'.repeat(64)},results:any[]=[];
 const approval=requestBoundApproval(store,{requestRef:'s10-binding',attribution:attribution as never,scope,reviewDigest:'f'.repeat(64),reviewed:[{field:'progress',effectRecordKey:'13',before:action.before,after:action.after}]});
 for(const [name,changed] of [['teacher',{target:{...target,teacherKey:'99002'}}],['student',{target:{...target,studentKey:'77002'}}],['date',{target:{...target,date:'2040-01-05'}}],['physical-record',{effectRecordKey:'99'}],['before',{before:'different'}],['after',{after:'different'}],['field',{field:'homework'}]] as const)results.push({name,decision:await approval.consumeApproval({...action,...changed} as never),ledgerCount:store.ledger(ledgerGroup(scope)).length});
 const capture=async(name:string,fn:()=>Promise<unknown>)=>{try{results.push({name,result:await fn()});}catch(e){results.push({name,code:(e as Error).message});}};
 const patch=[{field:'progress' as const,value:action.after,before:action.before,wireDigest:action.wireDigest}];
 for(const [name,s] of [['namespace',{...scope,sourceNamespace:'foreign'}],['native-teacher',{...scope,teacherId:'99002'}],['native-student',{...scope,studentId:'77002'}]] as const)await capture(name,()=>backend.saveDailyRecord(s,patch,approval));
 let consumes=0,dispatches=0;const state={target,fields,previousRecordKey:'12',previousDate:'2040-01-02',wireDigest:action.wireDigest};const port={read:async()=>state,consumeApproval:async()=>{consumes++;return {status:'rejected' as const};},write:async()=>{dispatches++;}};
 await capture('http-target-binding',()=>applyJournalHttpAction({...action,target:{...target,courseKey:'99'}} as never,port));
 await capture('http-physical-binding',()=>applyJournalHttpAction({...action,effectRecordKey:'99'} as never,port));
 await capture('http-before-drift',()=>applyJournalHttpAction({...action,before:'different'} as never,port));
 await capture('http-noop',()=>applyJournalHttpAction({...action,after:action.before} as never,port));
 const first=await approval.consumeApproval(action),duplicate=await approval.consumeApproval(action);
 output({results,first,duplicate,consumes,dispatches,ledger:store.ledger(ledgerGroup(scope))});
}
else throw Error('s10_unknown_reference_operation');
