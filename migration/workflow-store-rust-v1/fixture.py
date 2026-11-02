"""Fictional migration inputs and ports. Never imported by the runtime library."""
import base64,copy,hashlib,json,os,pathlib,re,selectors,signal,subprocess,time
import sys
sys.dont_write_bytecode=True
HERE=pathlib.Path(__file__).resolve().parent
TIME='2040-01-04T12:00:00.000Z'
SCOPE={'sourceNamespace':'academy.lms.dayrecord','teacherId':'99001','studentId':'77001','lessonDate':'2040-01-04'}
TARGET={'studentName':'Invented S10 Student','date':SCOPE['lessonDate'],'groupId':'3','teacherKey':'99001','studentKey':'77001','courseKey':'5','recordKey':'13','membershipKey':'8'}
FIELDS={'progress':'- 기존 진도','homework':'- 기존 숙제','lessonMemo':'','attendance':'Y','dailyTestAssessment':'0','previousHomeworkAssessment':'3'}
ATTR={'schemaVersion':'academy.dailyrecord-attribution/1','observedBy':'upstream','observedAt':TIME,'client':{'name':'s10-synthetic-owner','version':'1'},'process':{'pid':4242,'ppid':4241,'uid':0,'parentCommand':'synthetic-runner','bridgeSha256':None},'herdrPaneId':None,'declared':{'runtime':'synthetic-reference','sessionRef':'s10-fixture'}}
PARENT={'sourceRef':'s10:fake-owner-parent-policy','policySha256':'3'*64,'safeguardSourceSha256':'4'*64}
LEARNING={'bundleSha256':'5'*64,'configSha256':'6'*64,'catalogSha256':'7'*64}
def js(x):return json.dumps(x,ensure_ascii=False,separators=(',',':'))
def hash(s):return hashlib.sha256(s.encode()).hexdigest()
def canonical(x):return json.dumps(x,ensure_ascii=False,sort_keys=True,separators=(',',':'))
def digest(x):return hash(canonical(x))
def load(p):return json.loads(p.read_text())
def put(p,x):p.parent.mkdir(parents=True,exist_ok=True,mode=0o700);p.write_text(js(x)+'\n');p.chmod(0o600)
def event(root,x):
 with (root/'events.jsonl').open('a') as f:f.write(js(x)+'\n')
 (root/'events.jsonl').chmod(0o600)
def initialize(root):
 root.mkdir(parents=True,exist_ok=True,mode=0o700)
 if not (root/'source.json').exists():put(root/'source.json',{'fields':FIELDS,'previousRecordKey':'12','previousDate':'2040-01-02','mode':'normal'})
def observation(s,scope=SCOPE):
 t={**scope,'courseId':'5','membershipId':'8','recordId':'13'}
 return {'scope':scope,'target':t,'previousTarget':{**t,'lessonDate':s['previousDate'],'recordId':s['previousRecordKey']},'observedAt':TIME,'sourceRevision':'1'*64,'wireDigest':'a'*64,'fields':{f:({'state':'known','value':s['fields'][f],'evidenceRefs':['s10:synthetic-source:'+f]} if isinstance(s['fields'].get(f),str) else {'state':'unknown','reasonCode':'s10_fixture_field_not_observed'}) for f in FIELDS},'toneSamples':[],'transmissionState':'unknown'}
def context(request='s10-request',field='progress',value='- 지정 문제 풀이 진행.',date=SCOPE['lessonDate']):
 return {'schemaVersion':'academy.dailyrecord-user-context/1','purpose':'Characterize original TS against an explicitly fictional owner.','currentRequestRef':request,'scope':{**SCOPE,'lessonDate':date},'selectedFields':[field],'memoAnswer':{'kind':'no_addition','requestRef':'s10-memo-answer'},'facts':[{'ref':'s10:fact:'+field,'field':field,'text':re.sub(r'^- ','',value),'origin':'user_supplied',**({'explicitValue':value} if field in ['attendance','dailyTestAssessment','previousHomeworkAssessment'] else {}),'requiredFragments':[],'questionCounts':[],'deadlines':[],'stageClaims':[]}]}
def proposal(c,value):return {'field':c['selectedFields'][0],'value':value,'factRefs':[c['facts'][0]['ref']],'decision':{'kind':'replace','requestRef':c['currentRequestRef']}}
def files(p):
 if not p.exists():return []
 return [{'path':str(f.relative_to(p)),'bytes':base64.b64encode(f.read_bytes()).decode()} for f in sorted(p.rglob('*')) if f.is_file()]
def events(root):return [json.loads(s) for s in (root/'events.jsonl').read_text().splitlines()] if (root/'events.jsonl').exists() else []
def snapshot(root):
 es=events(root);return {'files':files(root/'store'),'source':load(root/'source.json'),'events':es,'downstream':{'pdf':sum(e['operation']=='pdf' for e in es),'print':sum(e['operation']=='print' for e in es),'parentSend':sum(e['operation']=='parent_send' for e in es)}}
def state(root):
 s=load(root/'source.json');return {'target':TARGET,'fields':s['fields'],'previousRecordKey':s['previousRecordKey'],'previousDate':s['previousDate'],'wireDigest':'a'*64}
def cism_report(root):
 s=load(root/'source.json');v=s.get('cismValue','');field={'fieldRef':'1'*64,'kind':'narrative','label':'Invented S10 narrative','value':v,'studentName':None,'groupName':None,'source':{'fieldName':'s_2'}}
 return {'date':'2040-10-15','periodStart':'2040-10-14','periodEnd':'2040-10-20','weekLabel':'Invented week','teacherLabel':'Invented S10 Teacher','teacherKey':SCOPE['teacherId'],'wireSha256':'2'*64,'reportSha256':hash(js([v])),'observedAt':TIME,'fields':[field],'sourceMode':'live_source','pdfGenerated':False}
class Peer:
 def __init__(self,root,options=None):self.root=root;self.options=options or {};self.reads=0;self.children=[]
 def port(self,name,v,child):
  root=self.root;s=load(root/'source.json');mode=s['mode']
  if name=='read_authoritative':event(root,{'operation':'read_authoritative','scope':v['scope']});return {'status':'observed','observation':observation(s,v['scope']),'subject':{'displayName':TARGET['studentName']}}
  if name=='parent_check_lines':return bool(re.fullmatch(r'- [^\r\n]+(?:\n- [^\r\n]+)*',v))
  if name=='parent_excluded':return False
  if name=='learning_scope_qualified':return True
  if name=='learning_review':return []
  if name=='learning_suggest':return None
  if name=='authenticate':event(root,{'operation':'fake_authenticate','teacherId':SCOPE['teacherId']});return None
  if name=='discover_groups':return {'groups':[{'groupId':'3'}]}
  if name=='execute':return {'dayRecords':[{**TARGET,'date':v['date'],**({} if v['date']==TARGET['date'] else {'recordKey':s['previousRecordKey']}),'fields':s['fields']}]}
  if name=='journal_state':return state(root)
  if name=='action_observed':
   event(root,{'operation':'action','action':v,'canonicalDigest':digest(v),'ownerDigest':digest(v)});return None
  if name=='approval_observed':event(root,{'operation':'consumed_approval','decision':v});return None
  if name in ('durable_index','durable_fence'):
   if self.options.get('crash')=={'durable_index':'index-only','durable_fence':'durable-fence'}[name]:return Abort({'durable_index':70,'durable_fence':71}[name])
   return None
  if name=='http_read':
   self.reads+=1
   if self.reads==2 and mode=='before-readback':return Abort(74)
   if self.reads==2 and mode=='readback-unavailable':raise PortError('s10_fake_readback_unavailable')
   return state(root)
  if name=='http_write':
   if mode=='before-dispatch':return Abort(72)
   event(root,{'operation':'fake_dispatch','field':v['field'],'effectRecordKey':v['effectRecordKey']})
   if mode=='timeout-old':raise PortError('s10_fake_timeout')
   if mode!='returned-old':
    s['fields'][v['field']]=v['after'];put(root/'source.json',s);event(root,{'operation':'fake_acceptance','field':v['field'],'effectRecordKey':v['effectRecordKey']})
   if mode=='after-acceptance':return Abort(73)
   if mode=='timeout-new':raise PortError('s10_fake_timeout')
   return None
  if name=='cism_read':return cism_report(root)
  if name=='cism_apply':
   event(root,{'operation':'fake_cism_dispatch','fieldRef':v['fieldRef']})
   if s.get('cismFail'):raise PortError('s10_fake_cism_timeout')
   s['cismValue']=v['after'];put(root/'source.json',s);return {'status':'persisted','fieldRef':v['fieldRef'],'report':cism_report(root)}
  if name=='current_user':return load(root/'current-user-event.json')
  if name=='lock_acquired':
   path=next((root/'store/locks').glob('*.flock'));st=path.stat();event(root,{'operation':'workflow_acquired','dev':st.st_dev,'ino':st.st_ino});(root/self.options.get('marker','held')).write_text('held');time.sleep(self.options.get('hold',0)/1000);return None
  raise RuntimeError('s10_unknown_port:'+name)
 def invoke(self,op,input,now=TIME,sub='store'):
  binary=os.environ['S10_RUST_BINARY'];root=self.root
  request={'operation':op,'root':str(root/sub),'ownerRoot':str(root),'now':now,'input':input,'parentPolicy':PARENT,'learningPolicy':LEARNING,'admission':{'teacherId':SCOPE['teacherId'],'teacherLabel':'Invented S10 Teacher','date':SCOPE['lessonDate'],'students':[{'studentId':SCOPE['studentId'],'displayName':TARGET['studentName']}],'sourceBundleSha256':'b'*64},'ownerAdmission':{'schemaVersion':'academy.owner-admission/2','decider':'user','decidedAt':'2040-01-04','sourceNamespace':SCOPE['sourceNamespace'],'teacherId':SCOPE['teacherId'],'allowedFrom':None,'allowedThrough':None,'failClosedDefault':'2026-10-23','studentScope':'source_verified'},'managedNames':[]}
  child=subprocess.Popen([binary],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,bufsize=0)
  starts=root/'rust-starts';starts.mkdir(mode=0o700,exist_ok=True);put(starts/(str(child.pid)+'.json'),{'pid':child.pid,'ppid':os.getpid(),'pgid':os.getpgid(child.pid),'operation':op,'binary':binary})
  start=time.monotonic();rawout=bytearray();rawerr=bytearray();pending=bytearray();sel=selectors.DefaultSelector();result=None;abort=None
  for name,p in [('stdout',child.stdout),('stderr',child.stderr)]:os.set_blocking(p.fileno(),False);sel.register(p,selectors.EVENT_READ,name)
  child.stdin.write((js(request)+'\n').encode());child.stdin.flush()
  try:
   while sel.get_map():
    if time.monotonic()-start>min(65,self.options.get('deadline',10)):raise RuntimeError('s10_rust_peer_deadline')
    for key,_ in sel.select(.02):
     data=os.read(key.fileobj.fileno(),65536)
     if not data:sel.unregister(key.fileobj);continue
     (rawout if key.data=='stdout' else rawerr).extend(data)
     if len(rawout)+len(rawerr)>4*1024*1024:raise RuntimeError('s10_rust_peer_output_bound')
     if key.data=='stderr':continue
     pending.extend(data)
     while b'\n' in pending:
      line,_,pending=pending.partition(b'\n');frame=json.loads(line)
      if 'port' in frame:
       try:
        value=self.port(frame['port'],frame['input'],child)
        reply={'abort':value.code} if isinstance(value,Abort) else {'value':value}
        if isinstance(value,Abort):abort=value.code
       except PortError as e:reply={'error':str(e)}
       child.stdin.write((js(reply)+'\n').encode());child.stdin.flush()
      elif 'result' in frame or 'error' in frame:
       if result is not None:raise RuntimeError('s10_duplicate_rust_result')
       result=frame
      else:raise RuntimeError('s10_rust_frame_invalid')
   child.wait(timeout=2)
   record={'command':[binary],'pid':child.pid,'returncode':child.returncode,'durationSeconds':time.monotonic()-start,'stdout':rawout.decode(),'stderr':rawerr.decode()}
   self.children.append(record)
   # Raw per-native-child evidence is outside the runtime journal snapshot.
   channel=root/'rust-channels';channel.mkdir(mode=0o700,exist_ok=True);put(channel/(str(child.pid)+'.json'),record)
   if rawerr or pending:raise RuntimeError('s10_rust_raw_channel_failure')
   if abort is not None:
    if child.returncode!=abort or result is not None:raise RuntimeError('s10_rust_fault_boundary_exit_mismatch')
    os._exit(abort)
   if child.returncode!=0 or result is None:raise RuntimeError('s10_rust_exit_failure')
   if 'error' in result:raise PortError(result['error']['message'])
   return result['result']
  finally:
   sel.close()
   if child.poll() is None:child.kill();child.wait(timeout=3)
   for stream in [child.stdin,child.stdout,child.stderr]:stream.close()
class PortError(Exception):pass
class Abort:
 def __init__(self,code):self.code=code
