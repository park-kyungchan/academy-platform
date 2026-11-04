"""Materialize fictional source inputs, invoke real Rust operations, use unchanged v1 judge.
No original semantic method supplies a Rust result. Only original prototype metadata is attached.
"""
import sys
sys.dont_write_bytecode=True
import json,pathlib,os,subprocess,time,hashlib,copy,base64,shutil,argparse
from native import module,run,ROOT
REF=ROOT/'migration/source-adapter-reference-v1'
SID='s09://account-A/student/0001'
AUDIT='private/student-audit-20261014/audit.json';LESSON='private/lesson-20261014/overlay.json';PIN='tools/academy-mcp/architecture/source-pins.json'
MAIN='/servlet/controller.cct.tutor.DayRecordServlet';SELECTED='/servlet/controller.cct.tutor.CourseScheduleServlet'
PRINCIPAL={'teacherLabel':'S09 Synthetic Teacher','teacherKey':'99001','collegeKey':'88001'}
TARGET={'studentName':'S09 Learner 가😀','date':'2040-01-04','groupId':'3','teacherKey':'99001','studentKey':'77001','courseKey':'5','recordKey':'13','membershipKey':'8'}
SCOPE={'sourceNamespace':'academy.lms.dayrecord','teacherId':'99001','studentId':'77001','lessonDate':'2040-01-04'}
REQUEST={'family':'day_record','teacherLabel':PRINCIPAL['teacherLabel'],'date':TARGET['date'],'groupIds':['3'],'studentNames':[TARGET['studentName']]}
ZERO={'liveAttempts':0,'socketAttempts':0,'childAttempts':0,'fakeDispatches':0,'deniedOperations':0,'mutatingFakeDispatches':0,'sourceHandlersExecuted':False}
def js(v):
 s=json.dumps(v,ensure_ascii=False,separators=(',',':'));out=[];i=0
 while i<len(s):
  n=ord(s[i])
  if 0xd800<=n<=0xdbff and i+1<len(s)and 0xdc00<=ord(s[i+1])<=0xdfff:out.append(chr(0x10000+((n-0xd800)<<10)+ord(s[i+1])-0xdc00));i+=2;continue
  out.append(('\\u%04x'%n)if 0xd800<=n<=0xdfff else s[i]);i+=1
 return ''.join(out)
def sha(b):return hashlib.sha256(b).hexdigest()
def text(n):return (REF/'corpus'/n).read_bytes().decode('utf8')
def error(message,code=None,name='Error'):
 e={'name':name,'message':message}
 if code:e['code']=code
 return {'outcome':'threw','error':e}
def returned(v):return {'outcome':'returned','value':v}
def primary(v):
 if isinstance(v,list):return [primary(x) for x in v]
 if isinstance(v,dict):
  out={k:primary(x) for k,x in v.items()}
  if v.get('outcome')=='threw':out['error']={k:primary(x) for k,x in v['error'].items() if k in ['name','message','issues','sourceOperation','httpStatus','code']}
  return out
 return v
class Driver:
 def __init__(self,binary,scratch):
  self.binary=str(binary);self.binary_hash=sha(pathlib.Path(binary).read_bytes());self.scratch=pathlib.Path(scratch);self.calls=[];self.effect=self.scratch/'effects.ndjson';self.effect.touch(mode=0o600);self.artifacts=self.scratch/'rust-calls';self.artifacts.mkdir(mode=0o700);self.counter=0
 def trace(self,v):
  with self.effect.open('a')as f:f.write(js(v)+'\n')
 def call(self,v):
  self.counter+=1;d=self.artifacts/f'{self.counter:04d}';d.mkdir(mode=0o700);raw=(js(v)+'\n').encode('utf8',errors='surrogatepass');(d/'stdin.bin').write_bytes(raw)
  start=time.monotonic();p=subprocess.Popen([self.binary],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE);pgid=os.getpgid(p.pid);(d/'START.json').write_text(js({'pid':p.pid,'pgid':pgid,'argv':[self.binary],'binarySha256':self.binary_hash})+'\n');out,err=p.communicate(raw,timeout=50);status={'pid':p.pid,'pgid':pgid,'argv':[self.binary],'binarySha256':self.binary_hash,'exitCode':p.returncode,'elapsedSeconds':time.monotonic()-start,'stdinSha256':sha(raw),'stdoutSha256':sha(out),'stderrSha256':sha(err)};(d/'stdout.bin').write_bytes(out);(d/'stderr.bin').write_bytes(err);(d/'RESULT.json').write_text(js(status)+'\n');self.calls.append(status)
  if p.returncode or err or out.count(b'\n')!=1:raise RuntimeError('rust_child_channels_invalid:'+str(d))
  return json.loads(out)
 def fixture(self,name,a=None,b=None):
  d=self.scratch/name;d.mkdir(mode=0o700)
  if a is None:a=json.loads(text('audit.json'))
  if b is None:b=json.loads(text('lesson.json'))
  for p,v in [(AUDIT,a),(LESSON,b)]:
   f=d/p;f.parent.mkdir(parents=True,exist_ok=True);f.write_bytes(v if isinstance(v,bytes) else (js(v)+'\n').encode());f.chmod(0o600)
  p=d/PIN;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(js({'files':[{'path':f,'sha256':sha((d/f).read_bytes())}for f in [AUDIT,LESSON]]}));p.chmod(0o600)
  return d
 def saved(self,d,q):return self.call({'op':'saved_query','root':str(d),'input':q})
 def native(self,variant,observer=False):
  adm={'teacherId':'99001','teacherLabel':PRINCIPAL['teacherLabel'],'date':TARGET['date'],'students':[{'studentId':'77001','displayName':TARGET['studentName']}],'sourceBundleSha256':'b'*64}
  target=copy.deepcopy(TARGET);row={**target,'fields':{'progress':'Current progress','homework':'','lessonMemo':'Memo','attendance':'Y','persistentMemo':'synthetic unexported'}};prior=copy.deepcopy(row);prior.update(date='2040-01-02',recordKey='12');prior['fields'].update(progress='Previous progress',homework='Previous homework');state={'target':target,'fields':{**row['fields'],'dailyTestAssessment':'0','previousHomeworkAssessment':'3'},'previousRecordKey':'12','previousDate':'2040-01-02','wireDigest':'a'*64}
  if observer:
   target['studentName']='S09 Observer Learner';row['studentName']=target['studentName'];prior['studentName']=target['studentName'];adm['students'][0]['displayName']=target['studentName'];fields={'progress':'','homework':'','lessonMemo':'','attendance':'Y','dailyTestAssessment':'0','previousHomeworkAssessment':'-1'};row['fields']=fields;prior['fields']=fields;state['fields']=fields
  if variant=='other-account':
   adm['teacherId']='99002';target['teacherKey']='99002';row['teacherKey']='99002';prior['teacherKey']='99002'
  if variant=='state-target':target['recordKey']='14'
  if variant in ['null-field','zero-field','missing-field','blank-field']:
   if variant=='missing-field':del state['fields']['progress']
   else:state['fields']['progress']={'null-field':None,'zero-field':0,'blank-field':''}[variant]
  if variant=='duplicate-current':current=[row,copy.deepcopy(row)]
  else:
   current=[row]
   for var,key,val in [('wrong-student','studentKey','77002'),('wrong-teacher','teacherKey','99002'),('wrong-date','date','2040-01-05')]:
    if variant==var:row[key]=val
  for var,key,val in [('previous-record','recordKey','11'),('previous-student','studentKey','77002'),('previous-course','courseKey','6'),('previous-member','membershipKey','9'),('previous-date','date','2040-01-01')]:
   if variant==var:prior[key]=val
  scope={**SCOPE,'teacherId':adm['teacherId']}
  for var,key,val in [('scope-namespace','sourceNamespace','foreign'),('scope-teacher','teacherId','99003'),('scope-student','studentId','77003'),('scope-date','lessonDate','2040-01-05')]:
   if variant==var:scope[key]=val
  fail=variant=='auth-failure-twice';auth={'operation':'authenticate',**({'error':{'name':'Error','message':'synthetic_auth_failure'}}if fail else {'value':None})}
  exchanges=[auth];iteration=[{'operation':'discover','value':{'groups':[{'groupId':'3'}]}},{'operation':'execute','value':{'dayRecords':current}},{'operation':'journalState','value':state},{'operation':'execute','value':{'dayRecords':[]if variant=='previous-missing'else[prior]}}]
  count=2 if variant in ['normal','auth-failure-twice']else 1
  if not fail:exchanges+=iteration*count
  result=self.call({'op':'native_session','admission':adm,'exchanges':exchanges,'scopes':[scope]*count})
  if result['outcome']!='returned':return result
  return returned({'fixtureAccount':'account-B'if variant=='other-account'else'account-A',**result['value'],'qualifiedProductionWire':False})
 def http(self,variant,auth_only):
  principal=copy.deepcopy(PRINCIPAL)
  for var,val in [('invalid-principal','0'),('constructor-leading-zero','099001'),('constructor-delimiter','99:001'),('constructor-whitespace','99001 '),('constructor-case-key','ABC')]:
   if variant==var:principal['teacherKey']=val
  recordings=[]
  def rec(path,method,selector,body='',status=200,headers=None,**kw):
   value={'path':path,'method':method,'selector':selector,'body':body,'status':status,'headers':headers or [],**kw}
   if path=='/servlet/controller.login.LoginServlet':value['credentialProof']={'userid':'s09-fictional-user','userpwd':'s09-fictional-password'}
   recordings.append(value)
  cookie='JSESSIONID=synthetic; '+('Domain=foreign.invalid; 'if variant=='foreign-cookie'else'')+'Path='+('/servlet/'if variant=='cookie-path'else'/')+'; HttpOnly'
  rec('/c_login.jsp','GET',None,'<form></form>'if variant=='login-wire'else text('login.html'),403 if variant=='login-failure'else 200,[['Set-Cookie',cookie]])
  redirect=variant in ['foreign-login-redirect','login-redirect','second-redirect','missing-location'];headers=[]if variant=='missing-location'else[['Location','https://foreign.invalid/'if variant=='foreign-login-redirect'else'/synthetic-login-next']]if redirect else [['Set-Cookie','JSESSIONID=; Max-Age=0; Path=/'if variant=='cookie-delete'else'OTHER=not-forwarded; Path=/']]
  rec('/servlet/controller.login.LoginServlet','POST','c_login',status=302 if redirect else 200,headers=headers)
  if variant in ['login-redirect','second-redirect']:rec('/synthetic-login-next','GET',None,status=302 if variant=='second-redirect'else 200,headers=[['Location','/synthetic-again']]if variant=='second-redirect'else[])
  principal_html=text('principal.html')
  if variant=='principal-key':principal_html=principal_html.replace('99001','99002')
  if variant=='principal-college':principal_html=principal_html.replace('88001','88002')
  rec('/servlet/controller.coursemanage.CourseManageServlet','GET','StudyCourse',principal_html)
  date='2040-01-02'if variant=='prior'else TARGET['date'];request={**REQUEST,'date':date}
  def source(path=MAIN):
   selected=path==SELECTED;body=text(('prior'if variant=='prior'else'current')+('.xml'if selected else'.html'))
   if not selected:
    if variant=='duplicate-rows':body=body.replace('</tbody>',body.split('<tbody>')[1].split('</tbody>')[0]+'</tbody>')
    if variant=='duplicate-date-id':body='<input id="duplicate" name="std_date" value="2040-01-04">'+body
    if variant=='broken-table':body=body.replace('</tbody></table>','</table>').replace('<tbody>','')
    for var,a,b in [('leading-zero-key',"'77001'","'077001'"),('case-name','S09 Learner','s09 Learner'),('space-name','S09 Learner',' S09 Learner'),('wrong-principal','99001','99002')]:
     if variant==var:body=body.replace(a,b)
   else:
    if variant=='inconsistent-key':body=body.replace('!77001!','!77002!')
    if variant=='malformed-xml':body='<root><record>'
   if variant=='login-challenge':body='<input type="password">'
   if variant=='oversize':body='x'*2000001
   kw={}
   if variant=='utf8-invalid':kw={'bodyBase64':base64.b64encode(body.encode()+b'\xff').decode()}
   if variant=='transport-throw':kw={'error':'synthetic_transport_failure'}
   if variant=='slow-abort':kw={'abortAfterDeadline':True}
   if variant=='stalled-body':kw={'stalled':True};body='<table>'
   if variant=='foreign-response-url':kw={'url':'https://foreign.invalid/returned'}
   rec(path,'GET'if selected else'POST','SelSglRecord'if selected else'Main',body,302 if variant=='redirect'else 503 if variant=='http-error'else 200,[['Location','/c_login.jsp']]if variant=='redirect'else[['Content-Type','text/html; charset=euc-kr'if variant=='source-encoding'else'text/xml'if selected else'text/html']],**kw)
  commands=[{'method':'authenticate','stopUnlessAuthenticated':True}]
  if variant=='retry-login':commands[0].pop('stopUnlessAuthenticated');commands.append({'method':'authenticate','stopUnlessAuthenticated':True})
  if not auth_only:
   if variant=='journal-unqualified':commands.append({'method':'journalState','input':TARGET});source();source(SELECTED);source()
   elif variant=='wire-inspection':commands.append({'method':'inspectWire','input':request});source()
   else:commands.extend([{'method':'discover','input':{'teacherLabel':PRINCIPAL['teacherLabel'],'date':date}},{'method':'execute','input':request}]);source();source();source(SELECTED);source(SELECTED)
  return self.call({'op':'http_session','principal':principal,'username':'s09-fictional-user','password':'s09-fictional-password','recordings':recordings,'commands':commands,'effectTrace':str(self.effect)})
 def special(self,c):
  variant=c['variant'];a=json.loads(text('audit.json'));b=json.loads(text('lesson.json'))
  if variant=='budget-partial':a['students'][0]['attempts']=[{'id':'budget-'+str(i),'payload':'가"\\'*12000}for i in range(8)]
  if variant=='budget-single':a['students'][0]['attempts']=[{'id':'oversized','payload':'가'*100000}]
  if variant=='budget-fixed':a['collectedAt']='x'*300000
  if variant=='loader-malformed-json':a=b'{'
  if variant=='loader-invalid-utf8':a=b'\xff'
  if variant=='loader-duplicate-id':a['students'][1]['studentId']=a['students'][0]['studentId']
  if variant=='loader-duplicate-name':a['students'][1]['name']=a['students'][0]['name']
  if variant=='loader-lesson-foreign':b['students'][0]['studentId']='outside'
  if variant=='loader-draft-foreign':b['students'][0]['dailyRecordDraft']['student']['studentId']='outside'
  if variant=='loader-event-duplicate':b['students'][0]['teacherEvents'].append(copy.deepcopy(b['students'][0]['teacherEvents'][0]))
  d=self.fixture(c['id'],a,b)
  if variant=='loader-pin-missing':(d/PIN).write_text('{"files":[]}')
  if variant=='loader-hash-mismatch':(d/AUDIT).write_text('{}')
  if variant=='loader-world-writable':(d/AUDIT).chmod(0o666)
  if variant=='loader-symlink':shutil.copyfile(d/AUDIT,d/'audit-copy.json');(d/AUDIT).unlink();(d/AUDIT).symlink_to(d/'audit-copy.json')
  if variant=='loader-parent-symlink':alias=d.with_name(c['id']+'-alias');alias.symlink_to(d);d=alias
  if variant.startswith('loader'):return returned(self.saved(d,{'query':{'kind':'roster'}}))
  if variant.startswith('budget'):return returned(self.saved(d,{'query':{'kind':'student_records','studentId':SID,'facet':'attempts'},'limit':250}))
  q={'kind':'student_history','studentId':SID};page=self.saved(d,{'query':q,'limit':2});results=[page['value']]
  if variant=='pagination':
   while page['value']['page']['nextCursor']:page=self.saved(d,{'query':q,'limit':2,'cursor':page['value']['page']['nextCursor']});results.append(page['value'])
   return returned(returned(results))
  cursor=page['value']['page']['nextCursor'];token=json.loads(base64.urlsafe_b64decode(cursor+'='*((4-len(cursor)%4)%4)))
  if variant=='cursor-wrong-selection':q={'kind':'roster'}
  if variant=='cursor-query-key-order':q={'studentId':SID,'kind':'student_history'}
  if variant=='cursor-wrong-snapshot':token['snapshot']='f'*64
  if variant=='cursor-offset-equal':token['offset']=page['value']['page']['totalInSavedSelection']
  if variant=='cursor-offset-over':token['offset']=page['value']['page']['totalInSavedSelection']+1
  if variant=='cursor-negative':token['offset']=-1
  if variant=='cursor-extra':token['extra']=True
  cursor=base64.urlsafe_b64encode(js(token).encode()).decode().rstrip('=');results.append(self.saved(d,{'query':q,'limit':3 if variant=='cursor-limit-change'else 2,'cursor':cursor})['value']);return returned(returned(results))
 def case(self,c):
  kind=c['kind'];variant=c.get('variant','normal')
  if kind in ['http','auth']:return self.http(variant,kind=='auth')
  if kind=='native':return self.native(variant)
  if kind=='saved':return self.saved(self.fixture(c['id']),c['input'])
  if kind=='saved-special':return self.special(c)
  if kind=='literal-key':return self.call({'op':'literal_key','value':c['value']})
  if kind=='resolve':return self.call({'op':'resolve_name','name':c['name'],'rows':c['rows']})
  if kind=='dom':return self.call({'op':'dom','html':text('dom.html')})
  if kind=='presentation':
   html=text('presentation.html');helper=text('helpers.js');extra={}
   if variant=='duplicate-control':html=html.replace('</select>','</select><select id="difficulty"><option>dup</option></select>',1)
   if variant=='duplicate-label':html='<label for="difficulty">duplicate</label>'+html
   if variant=='malformed-script':html=html.replace('var g_examtype_filter="all"','var g_examtype_filter=')
   if variant=='duplicate-function':helper+='\nfunction previewDBExam(key){return "another";}'
   if variant=='oversize':html='x'*2000001
   if variant=='too-many-options':html='<select id="difficulty">'+'<option>x</option>'*101+'</select>'
   if variant=='invalid-range':extra={'sourceRangeOverride':{'start':-1,'end':0}}
   return self.call({'op':'presentation','html':html,'helpers':helper,**extra})
  if kind=='paginate':return self.call({'op':'paginate','rows':[]if variant=='empty'else['a','b','c'],'query':{'kind':'synthetic'},'snapshot':'a'*64,'limit':{'limit-zero':0,'limit-fraction':1.5,'limit-max':250}.get(variant,3),**({'byteBudget':{'bad-budget':0,'byte-limited':7,'single-too-big':3}[variant]}if variant in ['bad-budget','byte-limited','single-too-big']else{})})
  if kind=='pinned-local':
   d=self.scratch/c['id'];d.mkdir(mode=0o700);p=d/'input.json';p.write_bytes(b'{"synthetic":true}\n');h=sha(p.read_bytes())
   if variant=='bad-hash':h='0'*64
   if variant=='world-writable':p.chmod(0o666)
   if variant=='oversize':p.write_bytes(b'x'*4194305)
   if variant=='directory':p=d
   if variant=='relative':p=pathlib.Path('input.json')
   if variant=='symlink':alias=d/'alias.json';alias.symlink_to(p);p=alias
   if variant=='ancestor-symlink':alias=d.with_name(c['id']+'-alias');alias.symlink_to(d);p=alias/'input.json'
   return self.call({'op':'pinned_local','path':str(p),'sha256':h})
  if kind=='native-config':return self.call({'op':'native_configuration_probe','args':[]if variant=='empty'else['--dailyrecord-native',str(self.scratch/'absent-config.json'),'b'*64]})
  if kind=='config-schema':return self.call({'op':'native_config_schema','input':{'schemaVersion':'academy.native-dailyrecord-read/1','sourceBundlePath':'/synthetic/bundle','sourceBundleSha256':'b'*64,'date':TARGET['date'],'students':[{'studentId':'77001','displayName':'S09 Learner'}]*2}})
  if kind=='paths':return self.call({'op':'legacy_paths'})
  if kind=='operation':
   field=c.get('field');post=field in ['dailyTestAssessment','previousHomeworkAssessment'];path='/servlet/controller.cct.common.CourseCommonServlet'if post else MAIN;method='POST'if post or not field else'GET';selector={'progress':'udtPrg','homework':'udtHw','lessonMemo':'udtMemo','attendance':'udtAttn','dailyTestAssessment':'SetDailyTest','previousHomeworkAssessment':'SetHomeWorkRate'}.get(field,'Main');selector='unreviewed'if variant=='unknown-selector'else selector;method='GET'if variant=='wrong-method'else method;key='reqCmd'if field else'p_process';pairs=[[key,selector]];query=pairs if field else[];body=[]if field else pairs
   if variant=='duplicate-selector':(query if field else body).append([key,selector])
   result=self.call({'op':'operation_admission','origin':'https://foreign.invalid'if variant=='foreign-origin'else'https://dc.gang-a.kr','path':path,'method':method,'query':query,'body':body});op={'method':method,'path':path,'selector':selector}
   if result['outcome']=='threw':self.trace({'event':'operation_denied',**op});return returned({**result,'operation':op,'dispatches':0,'basis':'original_mutating_field_contract'if field else'original_read_plan'})
   self.trace({'event':'operation_fake_dispatch',**op});return returned(returned({'operation':result['value'],'dispatches':1}))
  raise RuntimeError('driver_kind_unimplemented:'+kind)
 def frames(self,cases):
  frames=[]
  for c in cases:
   if c.get('variant')=='stalled-body':continue
   self.trace({'event':'case_start','id':c['id']});before=self.effect.stat().st_size;result=primary(self.case(c));events=[json.loads(s)for s in self.effect.read_bytes()[before:].splitlines()];effects={**ZERO,'fakeDispatches':sum(e['event']in ['recorded_dispatch','operation_fake_dispatch']for e in events),'deniedOperations':sum(e['event']=='operation_denied'for e in events)};frames.append({'id':c['id'],'kind':c['kind'],**result,'effects':effects});self.trace({'event':'case_end','id':c['id'],'effects':effects})
  return frames
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--binary',required=True);p.add_argument('--scratch',required=True);p.add_argument('--metadata',required=True);p.add_argument('--case');a=p.parse_args();d=Driver(a.binary,a.scratch);cases=json.loads((REF/'corpus/CASES.json').read_text());cases=[c for c in cases if c['id']==a.case]if a.case else cases;frames=d.frames(cases) if a.case!='http-stalled-body' else d.case(cases[0]);metadata=json.loads(pathlib.Path(a.metadata).read_text());assert metadata['owner']=='original-javascript-prototype-metadata-only'and metadata['effects']==ZERO
 print(js({'schema':'s09.original-reference-output/1','frames':frames,'runtimeExports':metadata['runtimeExports']}));(d.scratch/'RUST-CHILDREN.json').write_text(json.dumps(d.calls,indent=2)+'\n')
