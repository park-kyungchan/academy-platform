#!/usr/bin/env python3
"""Deliberately faulty processes for the migration observer; never a runtime participant."""
import sys
sys.dont_write_bytecode=True
from fixture import *
mode,root_s,raw_s=sys.argv[1:];root=pathlib.Path(root_s);frame=load(pathlib.Path(raw_s));data=frame['data']
if mode=='effect-only':event(root,{'operation':'parent_send'})
elif mode=='source-only':s=load(root/'source.json');s['fields']['progress']='unreported effect';put(root/'source.json',s)
elif mode=='extra-store':(root/'store'/'unexpected.bin').write_bytes(b'not in child snapshot')
elif mode=='extra-case':(root/'outside-snapshot.txt').write_text('extra')
elif mode=='extra-suite':(root.parent.parent/'outside-evidence.txt').write_text('extra')
elif mode=='denied':(root/'denied.log').write_text('denied_transport\n')
elif mode=='outcome-tamper':
 p=next((root/'store/outcome').glob('*.json'));v=load(p);v['record']['status']='unresolved';put(p,v);data['snapshot']=snapshot(root)
elif mode=='execution-removed':next((root/'store/execution').glob('*.json')).unlink();data['snapshot']=snapshot(root)
elif mode=='false-result':data['result']['public']['status']='unavailable'
elif mode=='downstream-lie':data['snapshot']['downstream']['parentSend']=1
elif mode=='extra-stdout':print('extraneous frame')
elif mode=='stderr':sys.stderr.write('fault diagnostic\n')
elif mode=='nonzero':sys.exit(7)
elif mode=='signal':os.kill(os.getpid(),signal.SIGTERM)
elif mode=='hang':time.sleep(30)
elif mode=='overflow':sys.stdout.write('X'*(5*1024*1024));sys.stdout.flush()
elif mode!='positive':raise RuntimeError('unknown probe')
print(js(data))
