#!/usr/bin/env python3
"""Bounded native command capture. Never computes candidate semantics."""
import os,sys,json,subprocess,time,pathlib,hashlib,signal
root=pathlib.Path('/home/palantirkc/work/ws-replan/preparation/slices/S08/core-fix-v4/native')
label=sys.argv[1]; args=sys.argv[2:]; out=root/label; out.mkdir(parents=True,exist_ok=False)
t=time.time()
p=subprocess.Popen(args,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=os.environ,start_new_session=True)
try:
 stdout,stderr=p.communicate(timeout=180)
 status=p.returncode;timed=False
except subprocess.TimeoutExpired:
 os.killpg(p.pid,signal.SIGKILL)
 stdout,stderr=p.communicate()
 status=p.returncode;timed=True
(out/'stdout').write_bytes(stdout);(out/'stderr').write_bytes(stderr)
r={'cwd':os.getcwd(),'argv':args,'pid':p.pid,'reaped':p.poll() is not None,'toolchain':os.environ.get('RUSTUP_TOOLCHAIN'),'jobs':os.environ.get('CARGO_BUILD_JOBS'),'target':os.environ.get('CARGO_TARGET_DIR'),'exit':status,'timeout':timed,'durationSeconds':time.time()-t,'stdoutSha256':hashlib.sha256(stdout).hexdigest(),'stderrSha256':hashlib.sha256(stderr).hexdigest()}
(out/'command.json').write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps(r));print(stdout.decode(errors='replace')[-12000:]);print(stderr.decode(errors='replace')[-8000:]);sys.exit(status if status is not None and status>=0 else 124)
