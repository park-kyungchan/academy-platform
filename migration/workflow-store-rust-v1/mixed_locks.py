#!/usr/bin/env python3
"""Real TS/Rust participants, same flock objects, cancellation/death and account order."""
import argparse,ctypes,fcntl,hashlib,json,os,pathlib,signal,subprocess,sys,time
HERE=pathlib.Path(__file__).resolve().parent;REF=HERE.parent/'workflow-store-reference-v1'
sys.dont_write_bytecode=True
sys.path.insert(0,str(REF));import run as original
from process_io import capture
from account_lock import reap_group,group_gone
from judge import canonical
from fixture import load,put
TS_LAUNCH=original.launch
sys.path.insert(0,str(HERE));import verify

def collect(p,out,label,expected=None,deadline=5):
 start=time.monotonic();stdout,stderr,timed,overflow=capture(p,deadline);adopted=reap_group(p.pid);gone=group_gone(p.pid)
 r={'label':label,'pid':p.pid,'returncode':p.returncode,'signal':-p.returncode if p.returncode<0 else None,'durationSeconds':time.monotonic()-start,'stdout':stdout.decode(),'stderr':stderr.decode(),'timedOut':timed,'outputBoundExceeded':overflow,'adoptedChildren':adopted,'groupGone':gone};put(out/(label+'.json'),r)
 if timed or overflow or stderr or not gone or expected is not None and p.returncode!=expected:raise RuntimeError('s10_mixed_channel_failure:'+label)
 return r

def ready(p,marker):
 start=time.monotonic()
 while not marker.exists():
  if p.poll() is not None or time.monotonic()-start>4:raise RuntimeError('s10_mixed_holder_not_ready')
  time.sleep(.01)
def blocked(fd):
 try:fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB);fcntl.flock(fd,fcntl.LOCK_UN);return False
 except BlockingIOError:return True
def identity(fd,path):
 a=os.fstat(fd);b=path.stat()
 if (a.st_dev,a.st_ino)!=(b.st_dev,b.st_ino):raise RuntimeError('s10_lock_path_object_changed')
 return [a.st_dev,a.st_ino]
def workflow(root,holder_kind):
 root.mkdir(parents=True,mode=0o700);launch=lambda kind,opts:(TS_LAUNCH if kind=='TS' else verify.launch)(root,'lock',json.dumps(opts))
 holder=launch(holder_kind,{'hold':60000,'marker':'held'});fd=None;waiter=None;cancel=None
 try:
  ready(holder,root/'held');path=next((root/'store/locks').glob('*.flock'));fd=os.open(path,os.O_RDWR);before=identity(fd,path)
  if not blocked(fd):raise RuntimeError('s10_retained_fd_not_contended')
  other='Rust' if holder_kind=='TS' else 'TS'
  waiter=launch(other,{'waitMs':140,'marker':'waiter'});r=collect(waiter,root,'other-language-deadline',0)
  if json.loads(r['stdout'])['result']['code']!='shared_writer_lock_busy' or (root/'waiter').exists():raise RuntimeError('s10_mixed_workflow_deadline_not_refused')
  cancel=launch(other,{'waitMs':30000,'started':'cancel-started','marker':'cancel-entered'});ready(cancel,root/'cancel-started');time.sleep(.1);os.killpg(cancel.pid,signal.SIGKILL);collect(cancel,root,'cancelled-waiter',-9)
  if not blocked(fd) or (root/'cancel-entered').exists():raise RuntimeError('s10_cancel_changed_holder_lock')
  os.killpg(holder.pid,signal.SIGKILL);collect(holder,root,'killed-holder',-9)
  fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
  if before!=identity(fd,path):raise RuntimeError('s10_workflow_kernel_object_replaced')
  fcntl.flock(fd,fcntl.LOCK_UN)
  recovered=launch(other,{'waitMs':500,'marker':'recovered'});r=collect(recovered,root,'other-language-recovered',0)
  if json.loads(r['stdout'])['result']['status']!='acquired' or before!=identity(fd,path):raise RuntimeError('s10_mixed_workflow_recovery')
  return {'holder':holder_kind,'waiter':other,'deadlineRefused':True,'waiterCancellation':True,'holderDeath':True,'retainedOpenDescriptor':True,'sameDeviceInode':before,'recovered':True}
 finally:
  if fd is not None:os.close(fd)
  for p in [holder,waiter,cancel]:
   if p and p.poll() is None:os.killpg(p.pid,signal.SIGKILL);p.communicate(timeout=3);reap_group(p.pid)

def account(root,binary):
 root.mkdir(parents=True,mode=0o700);p=root/'cases/account';p.mkdir(parents=True,mode=0o700)
 config=load(REF/'ACCOUNT-SOURCE.json');prefix=config['selectedPrefixes']['dailyrecord'];exe=config['flockExecutable']
 if hashlib.sha256(pathlib.Path(exe).read_bytes()).hexdigest()!=config['flockExecutableSha256'] or prefix['prefix'][:2]!=['--wait','45']:raise RuntimeError('s10_selected_account_prefix_changed')
 lock=p/'native-academy-account.lock'
 def launch(kind,opts):
  env={'PATH':'/usr/local/bin:/usr/bin:/bin','S10_EVIDENCE_ROOT':str(root),'S10_DENIED_LOG':str(p/'denied.log'),'TZ':'UTC','LANG':'C.UTF-8','S10_RUST_BINARY':binary,'PYTHONDONTWRITEBYTECODE':'1'}
  child=[original.BUN,'--no-env-file','--preload',str(REF/'deny.ts'),str(REF/'child.ts')] if kind=='TS' else [sys.executable,'-B',str(HERE/'child.py')]
  return subprocess.Popen([exe,*prefix['prefix'][:2],str(lock),*child,'lock',str(p),json.dumps(opts)],cwd=p,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
 holder=launch('TS',{'hold':60000,'marker':'held','started':'holder-ts-started'});waiter=None;fd=None
 try:
  ready(holder,p/'held');fd=os.open(lock,os.O_RDWR);before=identity(fd,lock);wf=next((p/'store/locks').glob('*.flock'));wf_before=[wf.stat().st_dev,wf.stat().st_ino]
  if not blocked(fd):raise RuntimeError('s10_account_did_not_contend')
  waiter=launch('Rust',{'waitMs':150,'marker':'waiter-entered','started':'waiter-ts-started'});start=time.monotonic();w=collect(waiter,root,'account-45-second-deadline',1,50);elapsed=time.monotonic()-start
  if w['stdout'] or not 44.5<=elapsed<49 or (p/'waiter-ts-started').exists() or (p/'waiter-entered').exists():raise RuntimeError('s10_account_deadline_or_order')
  os.killpg(holder.pid,signal.SIGKILL);h=collect(holder,root,'killed-ts-account-holder',-9)
  fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
  if before!=identity(fd,lock):raise RuntimeError('s10_account_kernel_object_replaced')
  fcntl.flock(fd,fcntl.LOCK_UN)
  recovered=launch('Rust',{'waitMs':500,'marker':'recovered','started':'recovered-ts-started'});r=collect(recovered,root,'recovered-account-and-workflow',0)
  if wf_before!=[wf.stat().st_dev,wf.stat().st_ino]:raise RuntimeError('s10_workflow_kernel_object_replaced')
  frame={'label':'recovered-account-and-workflow','returncode':0,'signal':None,'stderr':'','data':json.loads(r['stdout'])}
  result={'accountPrefix':{'command':'flock','flags':['--wait','45'],'lock':'<disposable-root>/native-academy-account.lock'},'accountDeadlineSeconds':45,'waiterReturncode':1,'waiterStdout':'','waiterStderr':'','waiterTSNotStarted':True,'accountBeforeWorkflow':True,'pythonSameDescriptorContended':True,'sameAccountKernelObjectAfterKill':True,'sameWorkflowKernelObjectAfterKill':True,'holderSignal':9,'holderAndDescendantsReaped':True,'adoptedChildrenSignals':[x['signal'] for x in h['adoptedChildren']],'recovered':original.normalize(frame),'liveTransport':0}
  diffs=verify.differences(result,load(REF/'ACCOUNT-EXPECTED.json'));put(root/'account-comparison.json',{'match':not diffs,'differences':diffs,'observedDeadlineSeconds':elapsed,'actualAccountObject':before,'actualWorkflowObject':wf_before})
  if diffs:raise RuntimeError('s10_mixed_account_expected_mismatch')
  # Reverse direction uses the same installed prefix; no redundant 45-second loop.
  (p/'held').unlink();holder=launch('Rust',{'hold':60000,'marker':'held','started':'rust-holder-started'});ready(holder,p/'held')
  waiter=launch('TS',{'waitMs':150,'marker':'reverse-entered','started':'reverse-ts-started'});time.sleep(.2)
  if (p/'reverse-ts-started').exists() or not blocked(fd):raise RuntimeError('s10_reverse_account_order')
  os.killpg(waiter.pid,signal.SIGKILL);collect(waiter,root,'cancelled-reverse-account-waiter',-9)
  os.killpg(holder.pid,signal.SIGKILL);collect(holder,root,'killed-rust-account-holder',-9)
  fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
  if before!=identity(fd,lock):raise RuntimeError('s10_reverse_account_object_changed')
  fcntl.flock(fd,fcntl.LOCK_UN)
  r=collect(launch('TS',{'waitMs':500,'marker':'reverse-recovered'}),root,'reverse-ts-recovered',0)
  return {'originalAccountExpectedMatch':True,'actualDeadlineSeconds':elapsed,'sourcePrefix45':True,'accountBeforeWorkflowBothDirections':True,'cancellation':True,'bothHolderDeaths':True,'retainedSameOpenDescriptor':True,'sameAccountObject':before,'sameWorkflowObject':wf_before,'allChildrenReaped':True}
 finally:
  if fd is not None:os.close(fd)
  for child in [holder,waiter]:
   if child and child.poll() is None:os.killpg(child.pid,signal.SIGKILL);child.communicate(timeout=3);reap_group(child.pid)
if __name__=='__main__':
 a=argparse.ArgumentParser();a.add_argument('--root',required=True,type=pathlib.Path);a.add_argument('--binary',required=True);args=a.parse_args()
 if ctypes.CDLL(None).prctl(36,1,0,0,0)!=0:raise RuntimeError('s10_subreaper_unavailable')
 verify.RUST_BINARY=str(pathlib.Path(args.binary).resolve());args.root.mkdir(mode=0o700,parents=True,exist_ok=False)
 results={'workflow':[workflow(args.root/'workflow-ts/cases/mixed-ts','TS'),workflow(args.root/'workflow-rust/cases/mixed-rust','Rust')],'account':account(args.root/'account',verify.RUST_BINARY)}
 put(args.root/'RESULT.json',results);print(json.dumps({'mixedWorkflowDirections':2,'mixedAccountDirections':2,'sourceDeadline45':True,'sameKernelObjects':True,'allChildrenReaped':True}))
