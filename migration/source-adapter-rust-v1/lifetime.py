"""Disposable lock passed to the actually stalled Rust child; no real account lock."""
import sys
sys.dont_write_bytecode=True
import argparse,fcntl,pathlib,subprocess,json,os
from compare import Driver,js
p=argparse.ArgumentParser();p.add_argument('--binary',required=True);p.add_argument('--scratch',required=True);a=p.parse_args();d=Driver(a.binary,a.scratch);lock=open(d.scratch/'fictional-lifetime.lock','w');fcntl.flock(lock,fcntl.LOCK_EX);original=d.call
def dispatch(v):
 raw=(js(v)+'\n').encode();(d.scratch/'stdin.bin').write_bytes(raw);child=subprocess.Popen([a.binary],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,pass_fds=(lock.fileno(),));(d.scratch/'CHILD.json').write_text(js({'pid':child.pid,'pgid':os.getpgrp(),'lockFdInherited':True})+'\n');out,err=child.communicate(raw);raise RuntimeError('stalled_child_returned')
d.call=dispatch;d.http('stalled-body',False)
