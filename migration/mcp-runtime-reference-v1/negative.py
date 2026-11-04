#!/usr/bin/env python3
"""Disposable actual failing-child and changed-source controls. Not a build."""
import argparse,json,pathlib,sys,hashlib
from observe import Child,save,pin
from judge import child_success
def main():
 a=argparse.ArgumentParser();a.add_argument('--output',required=True,type=pathlib.Path);x=a.parse_args();base=x.output.resolve();base.mkdir(parents=True,exist_ok=False,mode=0o700)
 inner=base/'inner.py';wrapper=base/'wrapper.py'
 inner.write_text('import os,sys,json,pathlib,time\np=pathlib.Path(sys.argv[1]);p.mkdir()\nfor n in ["stdin","stdout","stderr"]:(p/(n+".bin")).write_bytes(b"")\n(p/"PROCESS.json").write_text(json.dumps({"pid":os.getpid(),"pgid":os.getpgrp(),"argv":sys.argv,"cwd":os.getcwd(),"observedAt":time.time(),"requestedExit":23}))\nraise SystemExit(23)\n')
 wrapper.write_text('import subprocess,sys,json\np=subprocess.run([sys.executable,sys.argv[1],sys.argv[2]],stdin=subprocess.DEVNULL,capture_output=True)\nprint(json.dumps({"childStatus":p.returncode,"outerClaim":"green"}))\nraise SystemExit(0)\n')
 env={'PATH':'/usr/local/bin:/usr/bin:/bin','LANG':'C.UTF-8','TZ':'UTC'}
 c=Child([sys.executable,str(wrapper),str(inner),str(base/'inner-raw')],base,env,'lying-wrapper')
 while c.p.poll() is None:c.pump()
 c.finish()
 raw=json.loads((c.dir/'stdout.bin').read_text());traces='\n'.join(p.read_text() for p in c.dir.glob('syscalls.*'))
 if raw['childStatus']!=23 or 'exited with 23' not in traces:raise RuntimeError('actual_child_failure_not_verified')
 before=base/'fictional-source.txt';before.write_text('fictional-green-source\n');green=pin(before)
 before.write_text('fictional-source-changed-after-green\n');after=pin(before)
 rejected_wrapper=not child_success(c.p.returncode,[raw['childStatus']],green['sha256'],green['sha256'])
 rejected_drift=not child_success(0,[0],green['sha256'],after['sha256'])
 if not rejected_wrapper or not rejected_drift:raise RuntimeError('negative_control_accepted')
 save(base/'RESULT.json',{'lyingWrapper':{'outer':c.p.returncode,'actualChild':raw['childStatus'],'rejected':rejected_wrapper},'sourceAfterGreen':{'green':green,'current':after,'rejected':rejected_drift},'scripts':{'inner':pin(inner),'wrapper':pin(wrapper)},'notRustOrRealBuild':True})
 print(json.dumps({'lyingWrapperRejected':rejected_wrapper,'sourceAfterGreenRejected':rejected_drift}))
if __name__=='__main__':main()
