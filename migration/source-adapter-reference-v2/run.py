"""Bounded native executor. All output/scratch remains beneath explicit task notes."""
import argparse,pathlib,os,subprocess,selectors,time,signal,json,hashlib,resource,ctypes,datetime
MAX_OUTPUT=16*1024*1024

def descendants(group):
 found=[]
 for p in pathlib.Path('/proc').iterdir():
  if p.name.isdigit():
   try:
    s=(p/'stat').read_text(); fields=s[s.rfind(')')+2:].split()
    if int(fields[2])==group:found.append({'pid':int(p.name),'state':fields[0]})
   except (FileNotFoundError,ProcessLookupError,PermissionError):pass
 return found

def execute(command,directory,timeout=90,output_limit=MAX_OUTPUT,cwd=None):
 directory=pathlib.Path(directory);directory.mkdir(mode=0o700,parents=True,exist_ok=False)
 work=directory/'scratch';work.mkdir(mode=0o700)
 command=[str(x).replace('@SCRATCH@',str(work)) for x in command]
 env={'PATH':'/usr/local/bin:/usr/bin:/bin','LANG':'C.UTF-8','TZ':'UTC','TMPDIR':str(work),'BUN_RUNTIME_TRANSPILER_CACHE_PATH':'0'}
 def limits():
  resource.setrlimit(resource.RLIMIT_CORE,(0,0));resource.setrlimit(resource.RLIMIT_FSIZE,(64*1024*1024,64*1024*1024));resource.setrlimit(resource.RLIMIT_CPU,(90,90))
 ctypes.CDLL(None,use_errno=True).prctl(36,1,0,0,0)  # Own executor only: reap orphaned fixture descendants.
 start_utc=datetime.datetime.now(datetime.timezone.utc).isoformat();start=time.monotonic();p=subprocess.Popen(command,cwd=cwd or work,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True,preexec_fn=limits)
 (directory/'START.json').write_text(json.dumps({'pid':p.pid,'pgid':p.pid,'argv':command,'timeoutSeconds':timeout,'outputLimit':output_limit,'cwd':str(cwd or work)},indent=2)+'\n')
 sel=selectors.DefaultSelector();sel.register(p.stdout,selectors.EVENT_READ,'stdout');sel.register(p.stderr,selectors.EVENT_READ,'stderr')
 channels={'stdout':bytearray(),'stderr':bytearray()};termination=None
 while sel.get_map():
  if time.monotonic()-start>timeout and termination is None:termination='deadline'
  if sum(map(len,channels.values()))>output_limit and termination is None:termination='output_limit'
  if termination:
   try:os.killpg(p.pid,signal.SIGKILL)
   except ProcessLookupError:pass
  for key,_ in sel.select(.05):
   data=os.read(key.fileobj.fileno(),65536)
   if data:
    if len(channels[key.data])<output_limit+65536:channels[key.data].extend(data)
   else:sel.unregister(key.fileobj);key.fileobj.close()
  if time.monotonic()-start>timeout+3:break
 rc=p.wait(timeout=3)
 # Cancel/reap every descendant even when the leader exited normally.
 remaining=descendants(p.pid)
 if remaining:
  try:os.killpg(p.pid,signal.SIGKILL)
  except ProcessLookupError:pass
  end=time.monotonic()+2
  while time.monotonic()<end and any(x['state']!='Z' for x in descendants(p.pid)):time.sleep(.02)
 for _ in range(100):
  try:
   reaped,_=os.waitpid(-p.pid,os.WNOHANG)
   if reaped==0:time.sleep(.01)
  except ChildProcessError:break
 final=descendants(p.pid)
 status={'startedAt':start_utc,'finishedAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),'exitCode':rc if rc>=0 else None,'signal':-rc if rc<0 else None,'termination':termination,'elapsedSeconds':round(time.monotonic()-start,3),'descendantsObservedBeforeCleanup':remaining,'remainingGroupMembers':final,'liveDescendantsRemaining':[x for x in final if x['state']!='Z']}
 for name,data in channels.items():
  (directory/(name+'.bin')).write_bytes(data);status[name]={'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()}
 (directory/'RESULT.json').write_text(json.dumps(status,indent=2)+'\n')
 return status

if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--output',required=True);p.add_argument('--source-root',required=True);p.add_argument('--reference-root',required=True);p.add_argument('--case');p.add_argument('--timeout',type=float,default=90);a=p.parse_args()
 command=['/usr/local/bin/bun','--no-env-file',a.reference_root+'/reference.ts','--source-root',a.source_root,'--reference-root',a.reference_root,'--scratch-root','@SCRATCH@']
 if a.case:command+=['--case',a.case]
 print(json.dumps(execute(command,a.output,a.timeout)))
