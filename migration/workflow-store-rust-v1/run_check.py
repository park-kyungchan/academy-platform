#!/usr/bin/env python3
"""Bounded local check with raw channels and actual process-group readback."""
import argparse,ctypes,hashlib,json,os,pathlib,signal,subprocess,sys,time
sys.dont_write_bytecode=True
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parents[1]/'workflow-store-reference-v1'))
from process_io import capture
p=argparse.ArgumentParser();p.add_argument('--notes',required=True,type=pathlib.Path);p.add_argument('--label',required=True);p.add_argument('--seconds',type=float,default=180);p.add_argument('command',nargs=argparse.REMAINDER);a=p.parse_args()
a.notes.mkdir(parents=True,exist_ok=True,mode=0o700);command=a.command[1:] if a.command[:1]==['--'] else a.command
if ctypes.CDLL(None).prctl(36,1,0,0,0)!=0:raise RuntimeError('s10_subreaper_unavailable')
start=time.monotonic();child=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
out,err,timed,overflow=capture(child,a.seconds,4*1024*1024)
adopted=[];reap_deadline=time.monotonic()+3
while True:
 try:pid,status=os.waitpid(-child.pid,os.WNOHANG)
 except ChildProcessError:break
 if pid:adopted.append({'pid':pid,'rawWaitStatus':status});continue
 if time.monotonic()>reap_deadline:raise RuntimeError('s10_check_descendant_not_reaped')
 time.sleep(.01)
try:os.killpg(child.pid,0);gone=False;os.killpg(child.pid,signal.SIGKILL)
except ProcessLookupError:gone=True
for name,data in [('stdout',out),('stderr',err)]: (a.notes/(a.label+'.'+name)).write_bytes(data)
result={'command':command,'pid':child.pid,'returncode':child.returncode,'signal':-child.returncode if child.returncode<0 else None,'durationSeconds':time.monotonic()-start,'timedOut':timed,'outputBoundExceeded':overflow,'groupGone':gone,'adoptedChildren':adopted,'channels':{n:{'bytes':len(d),'sha256':hashlib.sha256(d).hexdigest()} for n,d in [('stdout',out),('stderr',err)]}}
(a.notes/(a.label+'.json')).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));print(err.decode()[-12000:]);raise SystemExit(0 if child.returncode==0 and gone and not timed and not overflow else 1)
