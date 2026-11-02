#!/usr/bin/env python3
"""Selected original flock launch prefix, original TS workflow lock, disposable files only."""
import argparse,ctypes,fcntl,hashlib,json,os,pathlib,signal,subprocess,time
from run import BUN,HERE,dump,group_gone,normalize
from judge import canonical
from process_io import capture
def reap_group(pid):
 statuses=[];deadline=time.monotonic()+3
 while True:
  try:child,status=os.waitpid(-pid,os.WNOHANG)
  except ChildProcessError:break
  if child:statuses.append({'pid':child,'rawWaitStatus':status,'signal':os.WTERMSIG(status) if os.WIFSIGNALED(status) else None});continue
  if time.monotonic()>deadline:raise RuntimeError('s10_account_descendant_not_reaped')
  time.sleep(.01)
 return statuses
def run(root):
 root.mkdir(parents=True,exist_ok=False,mode=0o700);p=root/'cases'/'account';p.mkdir(parents=True,mode=0o700)
 libc=ctypes.CDLL(None,use_errno=True)
 if libc.prctl(36,1,0,0,0)!=0:raise RuntimeError('s10_subreaper_unavailable')
 config=json.loads((HERE/'ACCOUNT-SOURCE.json').read_text());prefix=config['selectedPrefixes']['dailyrecord'];binary=config['flockExecutable']
 if hashlib.sha256(pathlib.Path(binary).read_bytes()).hexdigest()!=config['flockExecutableSha256']:raise RuntimeError('s10_flock_binary_changed')
 assert prefix['command']=='flock' and prefix['prefix'][:2]==['--wait','45']
 lock=p/'native-academy-account.lock';env={'PATH':'/usr/local/bin:/usr/bin:/bin','S10_EVIDENCE_ROOT':str(root),'S10_DENIED_LOG':str(p/'denied.log'),'TZ':'UTC','LANG':'C.UTF-8'}
 def launch(opts):return subprocess.Popen([binary,*prefix['prefix'][:2],str(lock),BUN,'--no-env-file','--preload',str(HERE/'deny.ts'),str(HERE/'child.ts'),'lock',str(p),json.dumps(opts)],cwd=p,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
 channels=[]
 def channel(proc,label,timeout):
  start=time.monotonic();out,err,timed,overflow=capture(proc,timeout)
  adopted=reap_group(proc.pid);gone=group_gone(proc.pid)
  x={'label':label,'pid':proc.pid,'returncode':proc.returncode,'signal':-proc.returncode if proc.returncode<0 else None,'stdout':out.decode(),'stderr':err.decode(),'elapsedSeconds':time.monotonic()-start,'timedOut':timed,'adoptedChildren':adopted,'groupGone':gone}
  dump(root/(label+'.channel.json'),x);channels.append(x)
  if timed or overflow or err or not gone:raise RuntimeError('s10_account_channel_failure:'+label)
  return x
 holder=launch({'hold':60000,'marker':'held','started':'holder-ts-started'});fd=None;waiter=None
 try:
  start=time.monotonic()
  while not (p/'held').exists():
   if holder.poll() is not None or time.monotonic()-start>4:raise RuntimeError('s10_account_holder_not_ready')
   time.sleep(.01)
  fd=os.open(lock,os.O_RDWR);before=os.fstat(fd);workflow=next((p/'store'/'locks').glob('*.flock'));wf_before=workflow.stat();blocked=False
  try:fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
  except BlockingIOError:blocked=True
  if not blocked:raise RuntimeError('s10_account_python_did_not_contend')
  waiter=launch({'waitMs':150,'marker':'waiter-entered','started':'waiter-ts-started'});started=time.monotonic();w=channel(waiter,'account-deadline',50);elapsed=time.monotonic()-started
  if w['returncode']!=1 or w['stdout'] or not 44.5<=elapsed<49:raise RuntimeError('s10_account_source_deadline_mismatch')
  if (p/'waiter-ts-started').exists() or (p/'waiter-entered').exists():raise RuntimeError('s10_workflow_before_account')
  os.killpg(holder.pid,signal.SIGKILL);h=channel(holder,'killed-account-holder',3)
  if h['returncode']!=-signal.SIGKILL:raise RuntimeError('s10_account_holder_signal')
  # The exact descriptor opened while the old owner was alive now acquires.
  fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB);after=os.fstat(fd)
  if (before.st_dev,before.st_ino)!=(after.st_dev,after.st_ino) or (lock.stat().st_dev,lock.stat().st_ino)!=(before.st_dev,before.st_ino):raise RuntimeError('s10_account_lock_replaced')
  fcntl.flock(fd,fcntl.LOCK_UN);os.close(fd);fd=None
  recovered=launch({'waitMs':500,'marker':'recovered','started':'recovered-ts-started'});r=channel(recovered,'recovered-account-and-workflow',5)
  if r['returncode']!=0:raise RuntimeError('s10_account_recovery_failed')
  obj=json.loads(r['stdout']);wf_after=workflow.stat()
  if (wf_before.st_dev,wf_before.st_ino)!=(wf_after.st_dev,wf_after.st_ino):raise RuntimeError('s10_workflow_lock_replaced')
  frame={'label':'recovered-account-and-workflow','returncode':0,'signal':None,'stderr':'','data':obj}
  result={'accountPrefix':{'command':'flock','flags':['--wait','45'],'lock':'<disposable-root>/native-academy-account.lock'},'accountDeadlineSeconds':45,'waiterReturncode':1,'waiterStdout':'','waiterStderr':'','waiterTSNotStarted':True,'accountBeforeWorkflow':True,'pythonSameDescriptorContended':True,'sameAccountKernelObjectAfterKill':True,'sameWorkflowKernelObjectAfterKill':True,'holderSignal':9,'holderAndDescendantsReaped':True,'adoptedChildrenSignals':[x['signal'] for x in h['adoptedChildren']],'recovered':normalize(frame),'liveTransport':0}
  if (p/'denied.log').exists():raise RuntimeError('s10_account_denied_transport')
  dump(root/'observations.json',{'accountIdentity':[before.st_dev,before.st_ino],'workflowIdentity':[wf_before.st_dev,wf_before.st_ino],'deadlineElapsedSeconds':elapsed,'children':[{k:v for k,v in c.items() if k not in ['stdout','stderr']} for c in channels]});dump(root/'transcript.json',result);return result
 finally:
  if fd is not None:os.close(fd)
  for proc in [holder,waiter]:
   if proc and proc.poll() is None:os.killpg(proc.pid,signal.SIGKILL);proc.communicate(timeout=3);reap_group(proc.pid)
if __name__=='__main__':
 a=argparse.ArgumentParser();a.add_argument('--root',required=True);a.add_argument('--freeze',action='store_true');args=a.parse_args();root=pathlib.Path(args.root).absolute();actual=run(root);expected=HERE/'ACCOUNT-EXPECTED.json'
 if args.freeze:
  if expected.exists():raise RuntimeError('s10_account_expected_exists')
  dump(expected,actual)
 elif canonical(actual)!=canonical(json.loads(expected.read_text())):raise RuntimeError('s10_account_transcript_mismatch')
 dump(root/'RESULT.json',{'sameAccountKernelObject':True,'sameWorkflowKernelObject':True,'accountBeforeWorkflow':True,'deadline':45,'allChildrenReaped':True,'frozenSha256':hashlib.sha256(expected.read_bytes()).hexdigest()});print(json.dumps({'sourceAccountDeadline':45,'sameKernelObjects':True,'accountBeforeWorkflow':True,'allChildrenReaped':True}))
