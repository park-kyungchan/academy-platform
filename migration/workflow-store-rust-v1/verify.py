#!/usr/bin/env python3
"""Unchanged original scenarios/comparator, Rust participant and independent artifact readback.
No expected data is sent to the participant. Normalization is the original v2 function;
actual native Rust PIDs are bound to their recorded logical participant only.
"""
import argparse,base64,ctypes,hashlib,json,os,pathlib,signal,subprocess,sys,time
HERE=pathlib.Path(__file__).resolve().parent;REF=HERE.parent/'workflow-store-reference-v1'
sys.dont_write_bytecode=True
sys.path.insert(0,str(REF));import run as original
from judge import canonical
from process_io import capture
from fixture import files,snapshot,load,put
ORIGINAL_COLLECT=original.collect;ORIGINAL_NORMALIZE=original.normalize;NATIVE_MAP={};RUST_BINARY=None

def launch(root,op,arg='s10-request'):
 env={'PATH':'/usr/local/bin:/usr/bin:/bin','S10_EVIDENCE_ROOT':str(root.parent.parent),'S10_DENIED_LOG':str(root/'denied.log'),'S10_RUST_BINARY':RUST_BINARY,'PYTHONDONTWRITEBYTECODE':'1','TZ':'UTC','LANG':'C.UTF-8'}
 return subprocess.Popen([sys.executable,'-B',str(HERE/'child.py'),op,str(root),arg],cwd=root,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
def collect(p,root,label,timeout=10,expected=0):
 out,err,timed,overflow=capture(p,timeout)
 adopted=[];deadline=time.monotonic()+3
 while True:
  try:
   pid,status=os.waitpid(-p.pid,os.WNOHANG)
  except ChildProcessError:break
  if pid:
   adopted.append({'pid':pid,'rawWaitStatus':status,'signal':os.WTERMSIG(status) if os.WIFSIGNALED(status) else None});continue
  if time.monotonic()>deadline:raise RuntimeError('s10_adopted_group_not_reaped')
  time.sleep(.01)
 if not original.group_gone(p.pid):raise RuntimeError('s10_descendant_group_remaining')
 raw={'label':label,'pid':p.pid,'returncode':p.returncode,'signal':-p.returncode if p.returncode<0 else None,'timedOut':timed,'outputBoundExceeded':overflow,'groupGone':True,'adoptedChildren':adopted,'stdout':out.decode(),'stderr':err.decode()};put(root/(label+'.channel.json'),raw)
 if overflow or timed or p.returncode!=expected or err:raise RuntimeError('s10_child_channel_failure:'+label+':'+str(p.returncode))
 if expected:
  if out:raise RuntimeError('s10_fault_unexpected_stdout')
  frame={'label':label,'returncode':p.returncode,'signal':raw['signal'],'stderr':'','stdout':''}
 else:
  if not out.endswith(b'\n') or len(out.splitlines())!=1:raise RuntimeError('s10_child_stdout_framing')
  frame={'label':label,'returncode':0,'signal':None,'stderr':'','data':json.loads(out)}
 native=[]
 channels=root/'rust-channels'
 if channels.exists():
  for f in sorted(channels.glob('*.json')):
   v=load(f);native.append({'path':str(f.relative_to(root)),'sha256':hashlib.sha256(f.read_bytes()).hexdigest(),'pid':v['pid'],'returncode':v['returncode']})
   NATIVE_MAP.setdefault(v['pid'],'<child:'+root.name+':'+label+'>')
 actual=snapshot(root)
 if 'data' in frame:
  if frame['data']['snapshot']!=actual:raise RuntimeError('s10_independent_source_journal_effect_readback_mismatch:'+label)
  result=frame['data'].get('result')
  if isinstance(result,dict) and 'files' in result:
   directory='cism' if frame['data']['op']=='cism' else 'consume'
   if result['files']!=files(root/directory):raise RuntimeError('s10_independent_additional_journal_readback_mismatch')
 # Inspect the entire declared suite root, including files outside the child snapshot.
 suite=root.parent.parent
 for entry in suite.iterdir():
  if entry.name not in ['cases','readbacks','transcript.json','comparison.json','RESULT.json']:
   raise RuntimeError('s10_unexpected_suite_artifact:'+entry.name)
 # Denied transport and unexpected files are separate from a plausible child snapshot.
 if (root/'denied.log').exists() and (root/'denied.log').read_bytes():raise RuntimeError('s10_denied_effect_readback')
 allowed={'source.json','events.jsonl','current-user-event.json','held','holder','waiter','recovered','holder-ts-started','waiter-ts-started','recovered-ts-started'}
 inventory=[]
 for f in sorted(root.rglob('*')):
  if not f.is_file():continue
  rel=f.relative_to(root);name=str(rel)
  if rel.parts[0] not in ['store','cism','consume','rust-channels','rust-starts'] and name not in allowed and not name.endswith('.input.json') and not name.endswith('.channel.json'):raise RuntimeError('s10_unexpected_artifact:'+name)
  inventory.append({'path':name,'bytes':f.stat().st_size,'sha256':hashlib.sha256(f.read_bytes()).hexdigest()})
 readbacks=root.parent.parent/'readbacks';readbacks.mkdir(mode=0o700,exist_ok=True)
 put(readbacks/(root.name+'--'+label+'.json'),{'outerPid':p.pid,'nativeChildren':native,'independentSnapshot':actual,'inventory':inventory,'deniedBytes':0,'groupGone':True})
 return frame
def normalize(frame,pid_map=None):return ORIGINAL_NORMALIZE(frame,{**(pid_map or {}),**NATIVE_MAP})
original.launch=launch;original.collect=collect;original.normalize=normalize

def differences(a,b,path='',out=None):
 out=[] if out is None else out
 if len(out)>=30:return out
 if type(a)!=type(b):out.append({'path':path,'actual':a,'expected':b});return out
 if isinstance(a,dict):
  for k in sorted(set(a)|set(b)):
   if k not in a or k not in b:out.append({'path':path+'/'+k,'actual':a.get(k,'<MISSING>'),'expected':b.get(k,'<MISSING>')})
   else:differences(a[k],b[k],path+'/'+k,out)
 elif isinstance(a,list):
  if len(a)!=len(b):out.append({'path':path+'/length','actual':len(a),'expected':len(b)})
  for i,(x,y) in enumerate(zip(a,b)):differences(x,y,path+'/'+str(i),out)
 elif a!=b:out.append({'path':path,'actual':a,'expected':b})
 return out
if __name__=='__main__':
 libc=ctypes.CDLL(None,use_errno=True)
 if libc.prctl(36,1,0,0,0)!=0:raise RuntimeError('s10_subreaper_unavailable')
 p=argparse.ArgumentParser();p.add_argument('--root',required=True,type=pathlib.Path);p.add_argument('--binary',required=True);p.add_argument('--suite',choices=['main','supplement','prepare'],default='main');a=p.parse_args();RUST_BINARY=str(pathlib.Path(a.binary).resolve());a.root.mkdir(parents=True,mode=0o700,exist_ok=False)
 if a.suite=='main':actual=original.run(a.root);expected=load(REF/'EXPECTED-v2.json')
 elif a.suite=='supplement':
  import supplement
  supplement.call=original.call;supplement.normalize=normalize
  # Original supplement creates its root itself.
  a.root.rmdir();actual=supplement.run(a.root);expected=load(REF/'SUPPLEMENT-EXPECTED.json')
 else:
  root=a.root/'cases'/'duplicate';root.mkdir(parents=True,mode=0o700);frame=original.call(root,'prepare','prepare',json.dumps({'requestRef':'s10-request','field':'progress','value':'- 지정 문제 풀이 진행.'}));actual=[{'scenario':'duplicate',**normalize(frame,{frame['data']['snapshot']['events'][0].get('pid',0):'unused'})}];expected=load(REF/'EXPECTED-v2.json')[:1]
 diff=differences(actual,expected);put(a.root/'comparison.json',{'match':not diff,'frames':len(actual),'differences':diff});print(json.dumps({'match':not diff,'frames':len(actual),'differencePaths':[d['path'] for d in diff]}));raise SystemExit(bool(diff))
