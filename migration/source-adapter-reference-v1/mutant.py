"""Actual disposable mutant children. No network, no real account/state."""
import sys,json,pathlib,os,fcntl,subprocess,time
mode,reference,result,scratch=sys.argv[1:];scratch=pathlib.Path(scratch)
if mode=='hung-descendant':
 lock=scratch/'fixture.lock';fd=os.open(lock,os.O_CREAT|os.O_RDWR,0o600);fcntl.flock(fd,fcntl.LOCK_EX)
 code='import os,time,pathlib;pathlib.Path('+repr(str(scratch/'descendant.pid'))+').write_text(str(os.getpid()));time.sleep(60)'
 p=subprocess.Popen([sys.executable,'-c',code],pass_fds=(fd,))
 (scratch/'effects.ndjson').write_text('{"event":"local_fake_dispatch","count":1,"settlement":"unknown"}\n')
 (scratch/'descendant-start.json').write_text(json.dumps({'pid':p.pid,'parent':os.getpid(),'lockSharedByInheritedFd':True}))
 print('local-fake-dispatched; child holds inherited fixture lock',flush=True);time.sleep(60);sys.exit(0)
if mode=='output-flood':
 while True:os.write(1,b'x'*65536)
if mode=='malformed-output':sys.stdout.write('{ malformed\n');sys.exit(0)
if mode=='signal':os.kill(os.getpid(),15)
base=pathlib.Path(result);data=json.loads((base/'stdout.bin').read_bytes());trace=(base/'scratch/effects.ndjson').read_bytes();(scratch/'effects.ndjson').write_bytes(trace)
frames={f['id']:f for f in data['frames']}
if mode.startswith('observer-'):
 if mode=='observer-category':data['frames'][0]['value']['first']['error']['category']='policy_denial'
 elif mode=='observer-cause':data['frames'][0]['value']['first']['error']['cause']['message']='different_cause'
 elif mode=='observer-account':data['frames'][1]['value']['sourceDistinguishesAccounts']=True
 elif mode=='observer-refusal':data['frames'][2]['error']['code']='different_refusal'
 elif mode=='observer-missing':del data['frames'][0]['value']['first']['error']['evidenceLocator']
 elif mode=='observer-effect':
  with (scratch/'effects.ndjson').open('ab') as f:f.write(b'{"event":"unintended_fake_dispatch","count":1}\n')
 elif mode=='observer-junk':print('unexpected stdout')
 elif mode!='observer-nonzero':raise SystemExit('unknown_observer_mutant')
 sys.stdout.write(json.dumps(data,ensure_ascii=False,separators=(',',':'))+'\n')
 sys.exit(7 if mode=='observer-nonzero' else 0)
if mode=='boolean-as-zero':frames['saved-summary']['value']['absenceProvesNonperformance']=0
elif mode=='broken-clock-relation':frames['native-normal']['value']['results'][0]['value']['observation']['toneSamples'][0]['observedAt']='2040-01-01T00:00:00.000Z'
elif mode=='wrong-value':frames['saved-summary']['value']['status']='complete'
elif mode=='missing-field':del frames['saved-summary']['value']['absenceProvesNonperformance']
elif mode=='changed-identity':frames['native-normal']['value']['results'][0]['value']['observation']['target']['studentId']='77002'
elif mode=='changed-effect':frames['http-normal']['effects']['fakeDispatches']+=1
elif mode=='changed-refusal':frames['presentation-invalid-range']['error']['message']='different_refusal'
elif mode=='changed-raw-span':frames['dom-values-and-exact-spans']['value']['spans'][0]['raw']+=' '
elif mode=='unintended-fake-dispatch':
 counter=0
 def dispatch():
  global counter
  counter+=1
  with (scratch/'effects.ndjson').open('ab') as f:f.write((json.dumps({'event':'unintended_fake_dispatch','count':counter})+'\n').encode())
 dispatch()
elif mode=='changed-source-account':frames['native-other-account']['value']['fixtureAccount']='account-A'
elif mode not in ['plausible-nonzero','junk-stdout','junk-stderr']:raise SystemExit('unknown_mutant')
if mode=='junk-stdout':print('unexpected debug')
if mode=='junk-stderr':print('unexpected diagnostic',file=sys.stderr)
sys.stdout.write(json.dumps(data,ensure_ascii=False,separators=(',',':'))+'\n')
if mode=='plausible-nonzero':sys.exit(7)
