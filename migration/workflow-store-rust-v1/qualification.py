#!/usr/bin/env python3
"""Real process faults and actual-file/effect mutations against independently read state."""
import argparse,ctypes,json,os,pathlib,subprocess,sys,base64,hashlib
sys.dont_write_bytecode=True
HERE=pathlib.Path(__file__).resolve().parent
sys.path.insert(0,str(HERE));import verify
from fixture import load,put,js
from judge import canonical
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--root',required=True,type=pathlib.Path);p.add_argument('--main-results',required=True,type=pathlib.Path);a=p.parse_args()
 if ctypes.CDLL(None).prctl(36,1,0,0,0)!=0:raise RuntimeError('s10_subreaper_unavailable')
 a.root.mkdir(mode=0o700,parents=True,exist_ok=False);raw=json.loads(load(a.main_results/'cases/duplicate/status.channel.json')['stdout']);baseline={'label':'probe','returncode':0,'signal':None,'stderr':'','data':raw};put(a.root/'baseline-actual-frame.json',baseline)
 results=[]
 modes=['positive','effect-only','source-only','extra-store','extra-case','extra-suite','denied','outcome-tamper','execution-removed','false-result','downstream-lie','extra-stdout','stderr','nonzero','signal','hang','overflow']
 for mode in modes:
  root=a.root/mode/'cases/state';root.mkdir(mode=0o700,parents=True)
  for f in raw['snapshot']['files']:
   p=root/'store'/f['path'];p.parent.mkdir(mode=0o700,parents=True,exist_ok=True);p.write_bytes(base64.b64decode(f['bytes']));p.chmod(0o600)
  put(root/'source.json',raw['snapshot']['source']);(root/'events.jsonl').write_text(''.join(js(e)+'\n' for e in raw['snapshot']['events']))
  env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1'};child=subprocess.Popen([sys.executable,'-B',str(HERE/'probe.py'),mode,str(root),str(a.root/'baseline-actual-frame.json')],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
  reason=None
  try:
   frame=verify.collect(child,root,'probe',timeout=.35 if mode=='hang' else 3)
   if canonical(frame)!=canonical(baseline):reason='actual_frame_mismatch'
  except RuntimeError as e:reason=str(e)
  accepted=reason is None
  results.append({'mode':mode,'accepted':accepted,'rejection':reason,'pid':child.pid,'returncode':child.returncode,'groupGone':verify.original.group_gone(child.pid)})
  if accepted!=(mode=='positive'):put(a.root/'partial-results.json',results);raise RuntimeError('s10_observer_control_wrong:'+mode)
 put(a.root/'RESULT.json',{'positive':1,'rejectedMutants':len(results)-1,'controls':results,'originalExpectedFilesReadByProbe':False,'allGroupsGone':all(r['groupGone'] for r in results)});print(json.dumps({'positive':1,'rejectedMutants':len(results)-1,'actualFileMutations':True,'actualNonzeroSignalHangOverflow':True,'allGroupsGone':True}))
