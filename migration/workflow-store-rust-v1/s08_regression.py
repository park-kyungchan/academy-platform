#!/usr/bin/env python3
"""Run the accepted unchanged S08 CLI regressions once using an explicit binary/output."""
import argparse,ctypes,hashlib,json,os,pathlib,subprocess,sys,time
sys.dont_write_bytecode=True
ROOT=pathlib.Path(__file__).resolve().parents[2];REF=ROOT/'migration/workflow-store-reference-v1';sys.path.insert(0,str(REF))
from process_io import capture
p=argparse.ArgumentParser();p.add_argument('--root',required=True,type=pathlib.Path);p.add_argument('--binary',required=True);a=p.parse_args();a.root.mkdir(mode=0o700,parents=True,exist_ok=False)
if ctypes.CDLL(None).prctl(36,1,0,0,0)!=0:raise RuntimeError('s10_subreaper_unavailable')
v4=ROOT/'migration/pure-semantics/candidate-v4'
commands=[('old202',['node',str(v4/'old202.mjs'),'--candidate',a.binary,'--results-root',str(a.root/'old202')]),('leaf54',['node',str(v4/'leaf54.mjs'),'--candidate',a.binary,'--results-root',str(a.root/'leaf54')]),('original15',['node',str(v4/'protocol-admission.mjs'),'--candidate',a.binary,'--cases','/home/palantirkc/work/ws-replan/preparation/slices/S08/PROTOCOL-ADMISSION-CASES.json','--results-root',str(a.root/'original15')]),('proposed13',['node',str(v4/'protocol-admission.mjs'),'--candidate',a.binary,'--cases',str(ROOT/'migration/pure-semantics/candidate-v3/admission-v3.json'),'--results-root',str(a.root/'proposed13')]),('new603',['node',str(v4/'compare.mjs'),'--candidate',a.binary,'--results',str(a.root/'new603')]),('controls',['node',str(v4/'controls.mjs'),'--actual',str(a.root/'new603'),'--old-actual',str(a.root/'old202'),'--results',str(a.root/'controls')])]
results=[]
for label,command in commands:
 start=time.monotonic();child=subprocess.Popen(command,cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True);out,err,timed,overflow=capture(child,120)
 try:os.killpg(child.pid,0);gone=False
 except ProcessLookupError:gone=True
 for name,data in [('stdout',out),('stderr',err)]:(a.root/(label+'.'+name)).write_bytes(data)
 r={'label':label,'command':command,'pid':child.pid,'returncode':child.returncode,'durationSeconds':time.monotonic()-start,'groupGone':gone,'timedOut':timed,'outputBoundExceeded':overflow,'stdoutSha256':hashlib.sha256(out).hexdigest(),'stderrSha256':hashlib.sha256(err).hexdigest()};results.append(r);(a.root/(label+'.json')).write_text(json.dumps(r,indent=2)+'\n')
 if child.returncode or err or timed or overflow or not gone:raise RuntimeError('s10_s08_regression_failure:'+label)
(a.root/'RESULT.json').write_text(json.dumps({'commands':results,'allPassed':True,'unchangedDriversAndOracles':True},indent=2)+'\n');print(json.dumps({'allPassed':True,'suites':['202','54','15','13','603','36+8+17 controls'],'allGroupsGone':True}))
