#!/usr/bin/env python3
"""Observe unchanged originals. One independent mode per invocation; no Rust.

Raw channels, Linux process/file trace and per-Bun-process channels are retained.
Expected outputs are produced separately by freeze.py after observation.
"""
import argparse,ctypes,datetime,hashlib,json,os,pathlib,resource,selectors,signal,subprocess,time
from pins import collect

HERE=pathlib.Path(__file__).resolve().parent
SHA=lambda b:hashlib.sha256(b).hexdigest()
MODE_DEADLINE=float('inf')
def save(p,v):
 p=pathlib.Path(p);p.parent.mkdir(parents=True,exist_ok=True,mode=0o700)
 p.write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n');p.chmod(0o600)
def pin(p):return {'path':str(p),'sha256':SHA(pathlib.Path(p).read_bytes())}
def members(group):
 result=[]
 for p in pathlib.Path('/proc').iterdir():
  if p.name.isdigit():
   try:
    s=(p/'stat').read_text().rsplit(')',1)[1].split()
    if int(s[2])==group:result.append({'pid':int(p.name),'state':s[0]})
   except (OSError,ValueError):pass
 return result

class Child:
 def __init__(self,argv,base,env,label):
  self.dir=base/'commands'/label;self.dir.mkdir(parents=True,mode=0o700)
  self.start=time.monotonic();self.deadline=min(self.start+90,MODE_DEADLINE);self.frames=[];self.buf=b'';self.total=0
  def limits():
   resource.setrlimit(resource.RLIMIT_CPU,(90,90));resource.setrlimit(resource.RLIMIT_FSIZE,(64*1024*1024,64*1024*1024));resource.setrlimit(resource.RLIMIT_CORE,(0,0))
  self.argv=['/usr/bin/strace','-ff','-ttt','-T','-s','4096','-e','trace=process,file,network','-o',str(self.dir/'syscalls'),*map(str,argv)]
  self.p=subprocess.Popen(self.argv,cwd=base,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True,preexec_fn=limits)
  save(self.dir/'START.json',{'argv':self.argv,'originalArgv':list(map(str,argv)),'pid':self.p.pid,'pgid':self.p.pid,'cwd':str(base),'startedAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),'limits':{'cpuSeconds':90,'wallSeconds':90,'fileBytes':64*1024*1024,'outputBytes':12*1024*1024},'environmentKeys':sorted(env)})
  self.sel=selectors.DefaultSelector()
  for n in ['stdout','stderr']:self.sel.register(getattr(self.p,n),selectors.EVENT_READ,n)
  self.raw={n:open(self.dir/(n+'.bin'),'xb') for n in ['stdin','stdout','stderr']};self.closed=False
 def send(self,v):
  b=v if isinstance(v,bytes) else (json.dumps(v,ensure_ascii=False,separators=(',',':'))+'\n').encode()
  self.raw['stdin'].write(b);self.raw['stdin'].flush();self.p.stdin.write(b);self.p.stdin.flush()
 def pump(self,wait=.05):
  if not self.closed and time.monotonic()>self.deadline:raise TimeoutError('s11_mode_deadline')
  for key,_ in self.sel.select(wait):
   b=os.read(key.fileobj.fileno(),65536)
   if not b:self.sel.unregister(key.fileobj);continue
   self.total+=len(b)
   if self.total>12*1024*1024:raise RuntimeError('s11_output_limit')
   self.raw[key.data].write(b);self.raw[key.data].flush()
   if key.data=='stdout':
    self.buf+=b
    while b'\n' in self.buf:
     line,self.buf=self.buf.split(b'\n',1)
     try:self.frames.append(json.loads(line))
     except ValueError:self.frames.append({'unparsedHex':line.hex()})
 def response(self,id,timeout=8):
  end=min(time.monotonic()+timeout,self.deadline)
  while time.monotonic()<end:
   for v in self.frames:
    if isinstance(v,dict) and v.get('id')==id:return v
   self.pump()
   if self.p.poll() is not None and not self.sel.get_map():break
  raise TimeoutError('s11_rpc_response_deadline')
 def rpc(self,id,method,params=None):
  v={'jsonrpc':'2.0','id':id,'method':method}
  if params is not None:v['params']=params
  self.send(v);return self.response(id)
 def init(self):
  self.rpc(0,'initialize',{'protocolVersion':'2025-11-25','capabilities':{},'clientInfo':{'name':'s11-fictional-observer','version':'1'}})
  self.send({'jsonrpc':'2.0','method':'notifications/initialized'})
 def finish(self,terminate=False):
  if self.closed:return
  self.closed=True
  try:self.p.stdin.close()
  except BrokenPipeError:pass
  if not terminate:
   end=min(time.monotonic()+1,self.deadline)
   while time.monotonic()<end and self.p.poll() is None:self.pump(.02)
  observed=members(self.p.pid)
  # Own traced group only; reap orphaned descendants before another mode.
  if self.p.poll() is None or observed:
   try:os.killpg(self.p.pid,signal.SIGTERM)
   except ProcessLookupError:pass
   end=time.monotonic()+.3
   while time.monotonic()<end:
    self.pump(.01)
    if self.p.poll() is not None:break
   if members(self.p.pid):
    try:os.killpg(self.p.pid,signal.SIGKILL)
    except ProcessLookupError:pass
  self.p.wait(timeout=2)
  while self.sel.get_map():self.pump(.01)
  for _ in range(200):
   try:
    pid,_=os.waitpid(-self.p.pid,os.WNOHANG)
    if pid==0:time.sleep(.005)
   except ChildProcessError:break
  for f in self.raw.values():f.close()
  save(self.dir/'RESULT.json',{'returncode':self.p.returncode,'elapsedSeconds':time.monotonic()-self.start,'cleanupRequested':terminate,'membersBeforeCleanup':observed,'membersAfterCleanup':members(self.p.pid),'channels':{n:pin(self.dir/(n+'.bin')) for n in self.raw}})

def main():
 global MODE_DEADLINE
 MODE_DEADLINE=time.monotonic()+90
 a=argparse.ArgumentParser();a.add_argument('--source-root',required=True,type=pathlib.Path);a.add_argument('--output',required=True,type=pathlib.Path);a.add_argument('--mode',required=True,choices=['inventory','discovery','protocol','composition','graph','identity','framing','bounds','launcher','relocation']);a.add_argument('--dependency-root',default='/home/palantirkc/projects/education-platform/modules/academy-workbench/tools/academy-mcp',type=pathlib.Path);x=a.parse_args()
 root=x.source_root.resolve();base=x.output.resolve();base.mkdir(parents=True,exist_ok=False,mode=0o700)
 ctypes.CDLL(None).prctl(36,1,0,0,0)
 snapshot=collect(root,x.dependency_root.resolve());mirror=base/'source';mirror.mkdir(mode=0o700)
 save(base/'PINS-before.json',snapshot)
 observer=base/'observer-source';observer.mkdir(mode=0o700)
 for name in ['observe.py','pins.py','preload.ts','fixture.ts']:(observer/name).write_bytes((HERE/name).read_bytes())
 save(base/'OBSERVER-PINS.json',{p.name:pin(p) for p in observer.iterdir()})
 for relative,identity in snapshot['sources'].items():
  p=mirror/relative;p.parent.mkdir(parents=True,exist_ok=True,mode=0o700);b=(root/relative).read_bytes()
  if SHA(b)!=identity['sha256']:raise RuntimeError('s11_source_changed_during_copy')
  p.write_bytes(b);p.chmod(0o600)
 (mirror/'node_modules').symlink_to(x.dependency_root.resolve()/'node_modules',target_is_directory=True)
 source_provenance={k:{**v,'originalPath':str(root/k),'executedPath':str(mirror/k)} for k,v in snapshot['sources'].items()}
 save(base/'SOURCE-MIRROR.json',source_provenance)
 fixture=mirror/'fictional-fault-fixture.ts';fixture.write_bytes((HERE/'fixture.ts').read_bytes())
 src=mirror/'modules/academy-workbench/tools/academy-mcp/src';tests=src.parent/'tests'
 env={'PATH':'/usr/local/bin:/usr/bin:/bin','LANG':'C.UTF-8','TZ':'UTC','TMPDIR':str(base),'BUN_RUNTIME_TRANSPILER_CACHE_PATH':'0','S11_SOURCE_ROOT':str(mirror),'S11_MODE_ROOT':str(base),'S11_DEPENDENCY_ROOT':str(x.dependency_root.resolve()),'NODE_PATH':str(x.dependency_root.resolve()/'node_modules'),'ACADEMY_MCP_INSTALL_DIR':str(base)}
 bun=pathlib.Path('/usr/local/bin/bun');preload=observer/'preload.ts'
 def argv(entry,*args):return [str(bun),'--no-install','--no-env-file','--preload',str(preload),str(entry),*map(str,args)]
 children=[];observations=[]
 def launch(entry,label,*args):
  c=Child(argv(entry,*args),base,env,label);children.append(c);return c
 admission={'schemaVersion':'academy.owner-admission/1','decider':'user','decidedAt':'2040-01-16','sourceNamespace':'academy.lms.dayrecord','teacherId':'99001','allowedFrom':'2040-01-15','allowedThrough':'2040-01-17','failClosedDefault':'2026-10-23'}
 save(base/'admission.json',admission);save(base/'routing.json',{'admissionPath':str(base/'admission.json'),'admissionSha256':pin(base/'admission.json')['sha256']})
 events=base/'events.txt'
 fault=x.mode in ['graph','bounds']
 endpoints={'dailyrecord':{'command':str(bun),'args':argv(fixture,'dailyrecord')[1:]} if fault else {'command':str(bun),'args':argv(tests/'neutral-upstream-fixture.ts',events)[1:]}}
 if x.mode in ['discovery','composition']:
  endpoints.update({'saved':{'command':str(bun),'args':argv(tests/'neutral-saved-fixture.ts')[1:]},'paper1':{'command':str(bun),'args':argv(tests/'neutral-paper-header-fixture.ts',events)[1:]},'paper2':{'command':str(bun),'args':argv(tests/'neutral-paper-header-fixture.ts',events)[1:]}})
 if x.mode=='bounds':endpoints['dailyrecord']={'command':'/usr/bin/flock','args':['--wait','1',str(base/'fictional-account.lock'),*argv(fixture,'dailyrecord')]}
 if x.mode=='identity':
  manifest={'schemaVersion':'academy.endpoint-identity/1','endpoint':'dailyrecord','semanticContractVersion':'s11-fictional-artifacts-only','launchArgumentsSha256':SHA(b'[]'),'artifacts':[{'role':'entry',**pin(src/'dailyrecord-server.ts')},{'role':'source',**pin(src/'dailyrecord.ts')}]}
  save(base/'endpoint-identity.json',manifest)
  identity=pin(base/'endpoint-identity.json')
  endpoints['dailyrecord']={'command':str(bun),'args':argv(src/'dailyrecord-server.ts','--runtime-identity',identity['path'],identity['sha256'])[1:]}
 for config in endpoints.values():config['env']=dict(env)
 save(base/'endpoints.json',endpoints)
 def pointer(generation='g1',version=1,**extra):
  v={'schemaVersion':f'academy.mcp-release/{version}','generation':generation,**{k:pin(base/(k+'.json')) for k in ['routing','endpoints','registry']},**extra}
  save(base/(generation+'.pointer.json'),v);os.replace(base/(generation+'.pointer.json'),base/'release.json');return v
 def call(c,id,name,args='OMIT'):
  p={'name':name}
  if args!='OMIT':p['arguments']=args
  r=c.rpc(id,'tools/call',p);observations.append({'id':id,'name':name,'response':r});return r
 try:
  if x.mode=='inventory':
   # Public installed tool descriptors only, not routing/admission payloads or actual endpoints.
   ip=pathlib.Path('/root/.local/share/academy-mcp-scoped-20261030/release.json');installed=json.loads(ip.read_text());rp=pathlib.Path(installed['registry']['path'])
   save(base/'installed-registry-provenance.json',{'pointer':pin(ip),'registry':pin(rp),'qualification':'public_metadata_replayed_by_unchanged_original_not_live_endpoint_inventory'})
   save(base/'registry.json',json.loads(rp.read_text()))
   # All descriptor routes target disposable endpoints; list never opens them.
   for entry in json.loads(rp.read_text()):endpoints.setdefault(entry['endpoint'],endpoints['dailyrecord'])
   save(base/'endpoints.json',endpoints)
  elif x.mode not in ['launcher','relocation']:
   d=launch(src/'neutral-server.ts','discover','--discover')
   end=time.monotonic()+15
   while d.p.poll() is None and time.monotonic()<end:d.pump()
   d.finish()
   if not (base/'registry.json').exists():raise RuntimeError('s11_discovery_setup_failed')
  if x.mode in ['launcher','relocation']:
   rel=base/'releases'/'fictional';rel.mkdir(parents=True,mode=0o700)
   # Source-preserving entry adapter: no protocol code is copied here.
   bridge=rel/'server.mjs';bridge.write_text('await import('+json.dumps(str(src/'dailyrecord-server.ts'))+');\n');bridge.chmod(0o600)
   # Preload inherited by the inner unchanged launcher child.
   env['BUN_OPTIONS']='--no-install --preload '+str(preload)
   save(rel/'bridge-pin.json',pin(bridge));save(base/'release.json',{'schemaVersion':'academy.mcp-release/1','generation':'fictional'})
   entry=src/'stable-launcher.ts'
   if x.mode=='relocation':
    entry=base/'relocated'/'launcher.ts';entry.parent.mkdir(mode=0o700);entry.write_bytes((src/'stable-launcher.ts').read_bytes())
   c=launch(entry,'valid-launch');c.init();observations.append({'id':'launcher-list','response':c.rpc(1,'tools/list',{})});c.finish()
   bridge.write_text('process.exit(99);\n')
   b=launch(entry,'tampered-bridge')
   while b.p.poll() is None:b.pump()
   b.finish()
   observations.append({'id':'tampered-bridge','status':b.p.returncode})
  else:
   c=launch(src/'neutral-server.ts','neutral');c.init();observations.append({'id':'tools-list','response':c.rpc(1,'tools/list',{})})
   if x.mode in ['inventory','discovery']:
    c.finish()
    if x.mode=='discovery':
     # Actual source entrypoint, backend deliberately absent and no native flags.
     u=launch(src/'dailyrecord-server.ts','dailyrecord-source');u.init();observations.append({'id':'dailyrecord-list','response':u.rpc(1,'tools/list',{})})
     for i,(name,args) in enumerate([('academy_endpoint_identity',{}),('academy_runtime_read_admission',{}),('academy_textbook_catalog',{'schoolLevel':'middle'}),('academy_cism_weekly_get',{'date':'2040-01-16'}),('unknown_fictional_tool',{}),('academy_runtime_read_admission',{'extra':1})],10):call(u,i,name,args)
     u.finish()
   elif x.mode=='protocol':
    call(c,2,'academy_runtime_identity',{});call(c,3,'unknown_fictional_tool',{});call(c,4,'academy_source_status');call(c,5,'academy_source_status',{'fictionalExtra':1})
    for i,reason in enumerate(['prestudy_search_form_drift','S11_FICTIONAL_PRIVATE_ERROR'],6):
     (base/'inject-source-status-error').write_text(reason);call(c,i,'academy_source_status',{})
    (base/'inject-source-status-error').unlink()
    req={'displayName':'Invented Student','purpose':'S11 fictional read','currentRequestRef':'s11-fictional-request','lessonDate':'2040-01-16'}
    call(c,8,'academy_dailyrecord_get',req);call(c,9,'academy_dailyrecord_get',{**req,'teacherId':'99002'})
    call(c,10,'academy_item_answer_lookup',{'labels':['12345'],'purpose':'fictional','currentRequestRef':'s11-fictional'})
    call(c,11,'academy_item_answer_lookup',{'labels':[str(12345+i) for i in range(33)],'purpose':'fictional','currentRequestRef':'s11-fictional'})
    (base/'inject-cism-transport-error').write_text('fictional');call(c,12,'academy_cism_weekly_save',{})
    observations.append({'id':'unknown-method','response':c.rpc(13,'fictional/method',{})});c.finish()
   elif x.mode=='composition':
    req={'displayName':'Invented Student','lessonDate':'2040-01-16','purpose':'S11 fictional composition','currentRequestRef':'s11-fictional','periods':['11']}
    call(c,2,'academy_dailytest_history_with_headers',req)
    for i,reason in enumerate(['source_authentication_failed','S11_FICTIONAL_PRIVATE_ERROR'],3):
     (base/'inject-metadata-error').write_text(reason);call(c,i,'academy_dailytest_history_with_headers',req)
    (base/'inject-metadata-error').unlink()
    # Same source routing function, owner-configured fictional student partition.
    routing=json.loads((base/'routing.json').read_text());routing['paperScopes']=[{'endpoint':'paper1','studentIds':['77002']},{'endpoint':'paper2','studentIds':['77001']}];save(base/'routing.json',routing);pointer()
    call(c,5,'academy_paper_headers',{'studentId':'77001','paperCodes':['7001']});call(c,6,'class2_academy_paper_headers',{'studentId':'77002','paperCodes':['7001']});call(c,7,'academy_paper_headers',{'studentId':'77999','paperCodes':['7001']});c.finish()
   elif x.mode=='identity':
    pointer('identity-v2',2,bridge=pin(src/'neutral-server.ts'),endpointIdentities={'dailyrecord':identity})
    call(c,2,'academy_runtime_identity',{})
    call(c,3,'academy_runtime_read_admission',{})
    u=launch(src/'dailyrecord-server.ts','qualified-endpoint','--runtime-identity',identity['path'],identity['sha256']);u.init();call(u,4,'academy_endpoint_identity',{});u.finish()
    (src/'dailyrecord.ts').write_bytes((src/'dailyrecord.ts').read_bytes()+b'\n// fictional disposable tamper\n')
    call(c,5,'academy_runtime_identity',{});c.finish()
   elif x.mode=='graph':
    # Existing legacy connection follows V1; preserve the observation as a known gap.
    pointer('g1');call(c,2,'academy_runtime_identity',{});call(c,3,'academy_source_status',{})
    pointer('g2');call(c,4,'academy_runtime_identity',{})
    save(base/'fixture-control.json',{'delayMs':500})
    c.send({'jsonrpc':'2.0','id':5,'method':'tools/call','params':{'name':'academy_cism_weekly_save','arguments':{}}})
    until=time.monotonic()+5
    while time.monotonic()<until:
     c.pump(.01)
     if (base/'fault-events.ndjson').exists() and 'pending' in (base/'fault-events.ndjson').read_text():break
    else:raise RuntimeError('s11_pending_not_observed')
    pointer('g3');observations.append({'id':5,'response':c.response(5)})
    save(base/'fixture-control.json',{})
    pointer('mismatch',2,bridge=pin(src/'dailyrecord-server.ts'),endpointIdentities={})
    call(c,6,'academy_source_status',{})
    pointer('recovered');call(c,7,'academy_source_status',{})
    (base/'release.json').unlink();call(c,8,'academy_source_status',{});c.finish()
   elif x.mode=='framing':
    c.send(b'{not-json}\n');c.send(b'{"jsonrpc":"2.0","id":8,"method":"tools/list","params":{}}\r\n');observations.append({'id':'crlf-after-malformed','response':c.response(8)})
    c.send(b'{"jsonrpc":"2.0","id":9,"method":"tools/list",');c.send(b'"params":{}}\n');observations.append({'id':'split-frame','response':c.response(9)})
    c.send(b'X'*(1024*1024+1)+b'\n');time.sleep(.1);c.pump();c.finish(True)
   elif x.mode=='bounds':
    call(c,2,'academy_bounded_probe',{'text':'four'});call(c,3,'academy_bounded_probe',{'text':'five!'});call(c,4,'academy_bounded_probe',{'text':'a','limit':4})
    save(base/'fixture-control.json',{'oversize':600000});call(c,5,'academy_source_status',{})
    save(base/'fixture-control.json',{'hang':True})
    c.send({'jsonrpc':'2.0','id':6,'method':'tools/call','params':{'name':'academy_cism_weekly_save','arguments':{}}})
    until=time.monotonic()+5
    while time.monotonic()<until:
     c.pump(.01)
     if (base/'fault-events.ndjson').exists() and 'hung' in (base/'fault-events.ndjson').read_text():break
    else:raise RuntimeError('s11_hung_dispatch_not_observed')
    try:c.response(6,timeout=1)
    except TimeoutError:observations.append({'id':'hung-after-possible-save-dispatch','observerDeadline':True,'outcome':'unknown','automaticRetryAllowed':False})
    c.finish(True)
    lock=Child(['/usr/bin/flock','--nonblock',str(base/'fictional-account.lock'),'/usr/bin/true'],base,env,'lock-release-readback');children.append(lock)
    while lock.p.poll() is None:lock.pump()
    lock.finish();observations.append({'id':'disposable-account-lock-after-cleanup','returncode':lock.p.returncode})
  save(base/'observations.json',observations)
  after=collect(root,x.dependency_root.resolve());save(base/'PINS-after.json',after)
  if snapshot!=after:raise RuntimeError('s11_original_or_installed_dependency_changed')
  save(base/'COMPLETE.json',{'mode':x.mode,'qualification':'unchanged_original_disposable_fictional_only','originalSourceRoot':str(root),'noRust':True,'observations':len(observations)})
 except BaseException as e:
  save(base/'FAILED.json',{'type':type(e).__name__,'code':str(e),'observationsCompleted':len(observations)})
  save(base/'partial-observations.json',observations);raise
 finally:
  for c in children:
   if not c.closed:c.finish(True)

if __name__=='__main__':main()
