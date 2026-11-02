#!/usr/bin/env python3
"""Migration-only fixture executor; all workflow decisions and journal writes use Rust."""
import sys
sys.dont_write_bytecode=True
from fixture import *
import sys
op,root_s,*tail=sys.argv[1:];arg=tail[0] if tail else 's10-request';root=pathlib.Path(root_s).absolute()
evidence=pathlib.Path(os.environ['S10_EVIDENCE_ROOT']).resolve()
if not root.is_relative_to(evidence) or root==evidence:raise RuntimeError('s10_owned_evidence_root_required')
initialize(root);peer=Peer(root,{'crash':arg} if op=='crash-save' else {})
def invoke(op,input,**kw):return peer.invoke(op,input,**kw)
def publish(kind,id,record):return invoke('store_publish',{'kind':kind,'id':id,'record':record})
def executions(scope=SCOPE):return invoke('executions',{'scope':scope})
def key(request,scope,field,record,before,after):return canonical({'requestRef':request,'student':canonical({k:scope[k] for k in ['sourceNamespace','teacherId','studentId']}),'field':field,'effectRecordKey':record,'beforeSha256':hash(before),'afterSha256':hash(after)})
def binding(action,request='s10-request',review='d'*64):return {'requestRef':request,'attribution':ATTR,'reviewDigest':review,'scope':SCOPE,'reviewed':[{'field':action['field'],'effectRecordKey':action['effectRecordKey'],'before':action['before'],'after':action['after']}]}
def action(field='progress',after='- 지정 문제 풀이 진행.',attempt='s10-binding',record='13',before=None):return {'attemptId':attempt,'target':TARGET,'field':field,'effectRecordKey':record,'before':FIELDS[field] if before is None else before,'after':after,'wireDigest':'a'*64}
def prepare(a):
 request=a.get('requestRef','s10-request');field=a.get('field','progress');value=a.get('value','- 지정 문제 풀이 진행.');date=a.get('date',SCOPE['lessonDate']);c=context(request,field,value,date);handle='dailyrecord-observation:s10-'+request
 publish('observation',handle,{'observation':observation(load(root/'source.json'),c['scope']),'scope':c['scope'],'requestRef':request,'subject':{'displayName':TARGET['studentName']},'createdBy':ATTR,'createdAt':TIME})
 proposals=[proposal(c,value)];review=invoke('review',{'context':c,'observationHandle':handle,'proposals':proposals,'attribution':ATTR})
 if review['status']!='prepared':raise RuntimeError('s10_fixture_review_not_prepared:'+js(review['rows']))
 input={'context':c,'observationHandle':handle,'proposals':proposals,'reviewDigest':review['reviewDigest'],'request':{'requestRef':request,'instruction':{'text':'Save exactly this reviewed fictional S10 field.','sessionRef':'s10-fixture','receivedAt':TIME}},'attribution':ATTR};put(root/(request+'.input.json'),input);return {'input':input,'review':review}
def tool_result(v):return {'content':[{'type':'text','text':js(v)}],'structuredContent':v}
def cism_call(op,v):
 try:return tool_result(invoke(op,v,sub='cism'))
 except PortError as e:
  code=str(e);safe=code in ['cism_prior_review_mismatch','cism_duplicate_field','cism_request_rebound','cism_uncertain_dispatch','cism_store_unqualified','cism_report_drift','cism_existing_value_preserved','cism_stale_before','cism_review_required','cism_readback_mismatch']
  public=code if safe else 'cism_source_failed'
  # Original public failure boundary, metadata only. Exact mapping is independently compared.
  category='upstream_unavailable';locator='academy-mcp:execution-boundary' if safe else 'academy-mcp:owner-read-boundary'
  if code in ['cism_prior_review_mismatch','cism_duplicate_field','cism_request_rebound','cism_report_drift','cism_stale_before']:category='contract_drift';locator='source-owner:response-contract'
  failure={'category':category,'code':public,'evidenceLocator':locator,'automaticRetryAllowed':False,'errorKind':'Error','errorSha256':hash('Error\0'+code)}
  return tool_result({'failure':failure,'academyWrites':None if op=='cism_save' else 0,'status':'failed','code':public,'automaticRetryAllowed':False,'dependentWork':'stop_and_diagnose','pdfGenerated':False,'parentSends':0})
if op=='prepare':result=prepare(json.loads(arg))
elif op=='mode':s=load(root/'source.json');s['mode']=arg;put(root/'source.json',s);result={'mode':arg}
elif op=='edit-source':s=load(root/'source.json');s.update(json.loads(arg));put(root/'source.json',s);result=s
elif op in ['save','crash-save']:result=invoke('save',load(root/((arg if op=='save' else 's10-request')+'.input.json')))
elif op=='edited-save':input=load(root/'s10-request.input.json');input.update(json.loads(arg));result=invoke('save',input)
elif op=='reconcile':
 input=load(root/'s10-request.input.json');options=json.loads(arg) if arg.startswith('{') else {};now=TIME
 if 'clock' in options:
  import datetime
  now=datetime.datetime.fromtimestamp(options['clock']/1000,datetime.timezone.utc).isoformat(timespec='milliseconds').replace('+00:00','Z')
 result=invoke('reconcile',{'context':input['context'],'attribution':ATTR,**({'resolution':options['resolution']} if 'resolution' in options else {})},now=now)
elif op=='status':
 scope=json.loads(arg) if arg.startswith('{') else SCOPE;es=executions(scope)
 unresolved=lambda f,r:[e for e in es if e['record']['field']==f and e['record']['effectRecordKey']==r and e['state'] in ['dispatch_outcome_unknown','reconciliation_required']]
 result={'public':invoke('status',{'scope':scope,'requestRef':'s10-request','attribution':ATTR}),'executions':es,'samePhysical':unresolved('progress','13'),'otherField':unresolved('homework','13'),'otherRecord':unresolved('progress','99')}
elif op=='seed-previous':
 a=action('previousHomeworkAssessment','4','s10-prior','12','3');result=invoke('approval',{'binding':binding(a,review='c'*64),'action':a})
elif op=='state-matrix':
 states=[]
 for name,kind,record in [('index-only',None,None),('fenced','execution',{}),('persisted','outcome',{'status':'persisted'}),('noop','outcome',{'status':'noop'}),('not-persisted','outcome',{'status':'not_persisted'}),('unresolved','outcome',{'status':'unresolved'}),('conflict','outcome',{'status':'conflict'}),('observed','reconciliation',{'classification':'persisted','basis':'readback_new_value'}),('resolved-old','reconciliation',{'classification':'not_persisted','basis':'user_instruction'}),('resolved-conflict','reconciliation',{'classification':'conflict','basis':'readback_after_returned_transport'})]:
  a=action(after='- 다음 진도',attempt='matrix-'+name);k=key('matrix-'+name,SCOPE,'progress','13',a['before'],a['after'])
  if name=='index-only':invoke('ledger_add',{'group':canonical({k:SCOPE[k] for k in ['sourceNamespace','teacherId','studentId']}),'key':k})
  else:
   invoke('approval',{'binding':binding(a,'matrix-'+name),'action':a})
   if kind!='execution':publish(kind,k,record)
  states.append({'name':name,'key':k,'state':invoke('field_state',{'key':k})})
 group=root/'store/ledger'/hash(canonical({k:SCOPE[k] for k in ['sourceNamespace','teacherId','studentId']}));ledger=sorted(load(p)['key'] for p in group.glob('*.json'))
 result={'states':states,'executions':executions(),'ledger':ledger}
elif op=='approval':
 a=action(attempt='s10-admission');current={'runtime':'s10-synthetic-owner','requestRef':'s10-user-request','requestTextSha256':hash('Synthetic explicit current user instruction.'),'currentUserEventRef':'s10:event:1'}
 admission={'schemaVersion':'academy.dailyrecord-save-admission/1',**current,'admitted':[{'studentId':SCOPE['studentId'],'lessonDate':SCOPE['lessonDate'],'field':'progress','effectRecordKey':'13','after':a['after']}]};put(root/'current-user-event.json',current)
 consume=lambda actions:invoke('launch_approval',{'admission':admission,'consumeRoot':str(root/'consume'),'currentUserPath':str(root/'current-user-event.json'),'actions':actions})
 foreign,foreign_record=consume([{**a,'target':{**a['target'],'studentKey':'77002'}},{**a,'effectRecordKey':'99'}]);put(root/'current-user-event.json',{**current,'currentUserEventRef':'s10:event:foreign'});stale=consume([a])[0];put(root/'current-user-event.json',current);first,duplicate=consume([a,a]);fresh=consume([a])[0]
 result={'current':current,'admission':admission,'action':a,'admissionDigest':hash(js(admission)),'foreign':foreign,'foreignRecord':foreign_record,'stale':stale,'first':first,'duplicate':duplicate,'freshObject':fresh,'consumed':files(root/'consume'),'expectedActionDigest':digest(a)}
elif op=='cism':
 (root/'cism').mkdir(mode=0o700,exist_ok=True);report=cism_report(root);f=report['fields'][0];k=hash(js([SCOPE['teacherId'],report['periodStart'],f['fieldRef']]))
 if arg=='seed-legacy':
  legacy={'fieldRef':f['fieldRef'],'before':'','after':'Legacy supported S10 narrative.','requestRef':'s10-legacy','reviewDigest':'e'*64};p=root/'cism'/(k+'.dispatch.json');p.write_text(js(legacy));p.chmod(0o600);result={'legacy':legacy,'key':k,'files':files(root/'cism')}
 elif arg=='get':result={'response':cism_call('cism_get',{'date':report['date']}),'files':files(root/'cism')}
 else:
  before=f['value'];input={'date':report['date'],'expectedReportSha256':report['reportSha256'],'request':{'requestRef':arg,'instruction':'Save only this exact supported fictional CISM narrative.'},'changes':[{'fieldRef':f['fieldRef'],'before':before,'after':before+' Additional supported S10 guidance.' if before else 'Supported S10 narrative.','edit':'complete_existing_narrative' if before else 'fill_empty','evidenceRefs':['s10:synthetic:dated-evidence']}],'attribution':ATTR}
  review=cism_call('cism_review',input);d=review.get('structuredContent',{}).get('reviewDigest');result={'input':input,'review':review,'response':cism_call('cism_save',{**input,'reviewDigest':d}) if d else None,'files':files(root/'cism')}
elif op=='review-held':
 c=context();handle='dailyrecord-observation:s10-unknown';s=load(root/'source.json');del s['fields']['progress'];put(root/'source.json',s)
 publish('observation',handle,{'observation':observation(s),'scope':SCOPE,'requestRef':c['currentRequestRef'],'subject':{'displayName':TARGET['studentName']},'createdBy':ATTR,'createdAt':TIME});result=invoke('review',{'context':c,'observationHandle':handle,'proposals':[proposal(c,'- 지정 문제 풀이 진행.')],'attribution':ATTR})
elif op=='binding-refusals':
 a=action();bind=binding(a,'s10-binding','f'*64);results=[]
 for name,change in [('teacher',{'target':{**TARGET,'teacherKey':'99002'}}),('student',{'target':{**TARGET,'studentKey':'77002'}}),('date',{'target':{**TARGET,'date':'2040-01-05'}}),('physical-record',{'effectRecordKey':'99'}),('before',{'before':'different'}),('after',{'after':'different'}),('field',{'field':'homework'})]:
  d=invoke('approval',{'binding':bind,'action':{**a,**change}});group=root/'store/ledger'/hash(canonical({k:SCOPE[k] for k in ['sourceNamespace','teacherId','studentId']}));results.append({'name':name,'decision':d,'ledgerCount':len(list(group.glob('*.json')))})
 patch=[{'field':'progress','value':a['after'],'before':a['before'],'wireDigest':a['wireDigest']}]
 for name,scope in [('namespace',{**SCOPE,'sourceNamespace':'foreign'}),('native-teacher',{**SCOPE,'teacherId':'99002'}),('native-student',{**SCOPE,'studentId':'77002'})]:
  try:results.append({'name':name,'result':invoke('native_save',{'scope':scope,'patches':patch,'binding':bind,'subject':None})})
  except PortError as e:results.append({'name':name,'code':str(e)})
 # These paths refuse/noop before any consumption or fake write, as in the original supplemental port.
 peer.options['binding_http']=True
 for name,change in [('http-target-binding',{'target':{**TARGET,'courseKey':'99'}}),('http-physical-binding',{'effectRecordKey':'99'}),('http-before-drift',{'before':'different'}),('http-noop',{'after':a['before']})]:
  try:results.append({'name':name,'result':invoke('http_action',{'binding':bind,'action':{**a,**change}})})
  except PortError as e:results.append({'name':name,'code':str(e)})
 first=invoke('approval',{'binding':bind,'action':a});duplicate=invoke('approval',{'binding':bind,'action':a});group=root/'store/ledger'/hash(canonical({k:SCOPE[k] for k in ['sourceNamespace','teacherId','studentId']}))
 result={'results':results,'first':first,'duplicate':duplicate,'consumes':0,'dispatches':0,'ledger':sorted(load(p)['key'] for p in group.glob('*.json'))}
elif op=='lock':
 options=json.loads(arg);peer.options=options
 if options.get('started'):(root/options['started']).write_text(str(os.getpid()))
 start=time.monotonic()
 try:invoke('lock',{'key':canonical({k:SCOPE[k] for k in ['sourceNamespace','teacherId','studentId']}),'waitMs':options.get('waitMs',180),'pollMs':10});result={'status':'acquired','elapsedMs':(time.monotonic()-start)*1000}
 except PortError as e:result={'status':'refused','code':str(e),'elapsedMs':(time.monotonic()-start)*1000}
 p=next((root/'store/locks').glob('*.flock'));s=p.stat();result['lock']={k:getattr(s,'st_'+k) for k in ['dev','mode','nlink','uid','gid','rdev','ino','size','blksize','blocks']}
 for k in ['atimeMs','mtimeMs','ctimeMs','birthtimeMs']:result['lock'][k]=0

elif op=='snapshot':result={}
else:raise RuntimeError('s10_unknown_migration_operation:'+op)
print(js({'op':op,'result':result,'snapshot':snapshot(root)}))
