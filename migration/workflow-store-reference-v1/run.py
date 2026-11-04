#!/usr/bin/env python3
"""Bounded original-source driver. Writes only beneath an explicitly new evidence root."""
import argparse,base64,hashlib,json,os,pathlib,re,signal,subprocess,sys,time
from process_io import capture
HERE=pathlib.Path(__file__).resolve().parent
BUN='/usr/local/bin/bun'
SCENARIOS=[]
def dump(p,x):p.write_text(json.dumps(x,ensure_ascii=False,indent=2)+'\n')
def load(p):return json.loads(p.read_text())
def group_gone(pid):
 try:os.killpg(pid,0);return False
 except ProcessLookupError:return True
def launch(root,op,arg='s10-request'):
 env={'PATH':'/usr/local/bin:/usr/bin:/bin','S10_EVIDENCE_ROOT':str(root.parent.parent),'S10_DENIED_LOG':str(root/'denied.log'),'TZ':'UTC','LANG':'C.UTF-8'}
 return subprocess.Popen([BUN,'--no-env-file','--preload',str(HERE/'deny.ts'),str(HERE/'child.ts'),op,str(root),arg],cwd=root,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
def collect(p,root,label,timeout=10,expected=0):
 out,err,timed,overflow=capture(p,timeout)
 if not group_gone(p.pid):
  os.killpg(p.pid,signal.SIGKILL)
  raise RuntimeError('s10_descendant_group_remaining')
 raw={'label':label,'pid':p.pid,'returncode':p.returncode,'signal':-p.returncode if p.returncode<0 else None,'timedOut':timed,'outputBoundExceeded':overflow,'groupGone':True,'stdout':out.decode(),'stderr':err.decode()}
 dump(root/(label+'.channel.json'),raw)
 if overflow:raise RuntimeError('s10_output_bound')
 if timed or p.returncode!=expected or err:raise RuntimeError('s10_child_channel_failure:'+label+':'+str(p.returncode))
 if expected:
  if out:raise RuntimeError('s10_fault_unexpected_stdout')
  return {'label':label,'returncode':p.returncode,'signal':raw['signal'],'stderr':'','stdout':''}
 if not out.endswith(b'\n') or len(out.splitlines())!=1:raise RuntimeError('s10_child_stdout_framing')
 obj=json.loads(out)
 # Independent effect readback, so a plausible result cannot hide a fake dispatch.
 log=root/'events.jsonl';events=[json.loads(s) for s in log.read_text().splitlines()] if log.exists() else []
 if obj['snapshot']['events']!=events:raise RuntimeError('s10_effect_channel_mismatch')
 denied=root/'denied.log'
 if denied.exists() and denied.read_bytes():raise RuntimeError('s10_denied_transport_effect')
 return {'label':label,'returncode':0,'signal':None,'stderr':'','data':obj}
def call(root,label,op,arg='s10-request',expected=0):
 root.mkdir(parents=True,exist_ok=True,mode=0o700)
 return collect(launch(root,op,arg),root,label,expected=expected)
def normalize(frame,pid_map=None):
 if 'data' not in frame:return frame
 data=frame['data'];events=data['snapshot']['events'];aliases={};n=0
 for e in events:
  if e.get('operation')=='action':
   a=e['action'];n+=1
   if not re.fullmatch(r'[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}',a['attemptId']):raise RuntimeError('s10_native_attempt_not_uuid')
   if e['canonicalDigest']!=e['ownerDigest']:raise RuntimeError('s10_original_action_digest_disagreement')
   aliases[a['attemptId']]=f'<attempt:{n}>';aliases[e['canonicalDigest']]=f'<action-digest:{n}>'
 def rec(x,key='',path=()):
  if isinstance(x,dict):
   result={k:rec(v,k,path+(k,)) for k,v in x.items()}
   if 'elapsedMs' in result:
    actual=x['elapsedMs']
    if not 0<=actual<4000:raise RuntimeError('s10_lock_deadline_bound')
    result['elapsedMs']='<bounded-monotonic-ms>'
   return result
  if isinstance(x,list):
   vals=[rec(v,key,path) for v in x]
   # Audit filename order inside a single clock millisecond is random UUID order,
   # not a guaranteed execution order. Preserve raw files; compare this multiset only.
   return sorted(vals,key=lambda v:json.dumps(v,sort_keys=True,ensure_ascii=False)) if key=='audit' else vals
  if isinstance(x,str):
   if x in aliases:return aliases[x]
   if key in ('recordedAt','reconciledAt','at'):
    if not re.fullmatch(r'\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{3}Z',x):raise RuntimeError('s10_wall_clock_shape')
    return '<wall-clock-iso>'
  if key=='pid' and isinstance(x,int) and pid_map and x in pid_map:return pid_map[x]
  return x
 result=rec(frame)
 def decode_files(items):
  normalized=[];audit=[]
  for f in items:
   p=f['path'];raw=base64.b64decode(f['bytes'])
   if not p.endswith('.json'):
    if p.endswith('.flock') and raw==b'':normalized.append({'path':p,'bytes':''});continue
    raise RuntimeError('s10_unexpected_store_file:'+p)
   parsed=json.loads(raw)
   if p.startswith('execution/') and parsed['record']['actionDigest'] in aliases:
    if parsed['record']['attemptId'] not in aliases:raise RuntimeError('s10_execution_attempt_binding')
   obj=rec(parsed)
   if p.startswith('audit/'):audit.append(obj);continue
   # Keep exact JSON serialization/key order/spacing/newline after only specified substitutions.
   text=raw.decode()
   for a,b in aliases.items():text=text.replace(a,b)
   for k in ('recordedAt','reconciledAt','at'):
    text=re.sub(r'("'+k+r'":")\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{3}Z(")',r'\1<wall-clock-iso>\2',text)
   normalized.append({'path':p,'text':text})
  if audit:normalized.append({'auditMultiset':sorted(audit,key=lambda v:json.dumps(v,sort_keys=True,ensure_ascii=False))})
  return normalized
 result['data']['snapshot']['files']=decode_files(data['snapshot']['files'])
 # CISM and launch-admission payloads have their own original byte representation.
 if isinstance(data['result'],dict) and 'files' in data['result']:
  result['data']['result']['files']=decode_files(data['result']['files'])
 # Lock identity is compared relationally by the controller below; retain raw dev/inode in channels.
 def lockrec(x):
  if isinstance(x,dict):
   return {k:('<same-disposable-device>' if k=='dev' else '<same-disposable-inode>' if k=='ino' else '<fs-time>' if k in ('atime','mtime','ctime','birthtime','atimeMs','mtimeMs','ctimeMs','birthtimeMs') else lockrec(v)) for k,v in x.items()}
  if isinstance(x,list):return [lockrec(v) for v in x]
  return x
 return lockrec(result)
def run(root):
 transcript=[];rawframes=[];pid_map={}
 def scenario(name):
  p=root/'cases'/name;p.mkdir(parents=True,mode=0o700);return p
 def c(p,label,op,arg='s10-request',expected=0):
  f=call(p,label,op,arg if isinstance(arg,str) else json.dumps(arg),expected);pid_map[load(p/(label+'.channel.json'))['pid']]='<child:'+p.name+':'+label+'>';rawframes.append((p,label,f));transcript.append({'scenario':p.name,**normalize(f,pid_map)});return f.get('data',{}).get('result')
 prep={'requestRef':'s10-request','field':'progress','value':'- 지정 문제 풀이 진행.'}
 p=scenario('duplicate');c(p,'prepare','prepare',prep);c(p,'save-first','save');c(p,'save-second-process','save');c(p,'edited-reviewed-input','edited-save',{'proposals':[{'field':'progress','value':'- 바뀐 문안','factRefs':['s10:fact:progress'],'decision':{'kind':'replace','requestRef':'s10-request'}}]});c(p,'instruction-conflict','edited-save',{'request':{'requestRef':'s10-request','instruction':{'text':'Different synthetic instruction.'}}});c(p,'status','status')
 p=scenario('uncertain-old');c(p,'prepare','prepare',prep);c(p,'mode','mode','timeout-old');c(p,'save-timeout','save');c(p,'reconcile-old','reconcile');c(p,'reconcile-years-later','reconcile',{'clock':2525299200000});c(p,'duplicate-blocked','save');c(p,'prepare-new-request','prepare',{**prep,'requestRef':'s10-new'});c(p,'new-request-blocked','save','s10-new');c(p,'status-different-date','status',{**load(p/'s10-request.input.json')['context']['scope'],'lessonDate':'2040-01-05'});c(p,'prepare-new-date','prepare',{**prep,'requestRef':'s10-new-date','date':'2040-01-05'});c(p,'new-date-blocked','save','s10-new-date')
 # The same fake source later independently contains the desired bytes. No action is replayed.
 s=load(p/'source.json');s['fields']['progress']=prep['value'];c(p,'external-observation','edit-source',s);c(p,'observe-desired','reconcile');c(p,'settled-no-replay','save')
 p=scenario('returned-old');c(p,'prepare','prepare',prep);c(p,'mode','mode','returned-old');c(p,'save','save');c(p,'same-request-consumed','save')
 p=scenario('backfill');c(p,'prepare','prepare',{**prep,'field':'previousHomeworkAssessment','value':'4'});c(p,'seed-previous','seed-previous');c(p,'backfill-binding','edit-source',{'previousRecordKey':'99','previousDate':'2040-01-03'});c(p,'reconcile-different-record','reconcile');c(p,'status','status')
 for name,code in [('index-only',70),('durable-fence',71),('before-dispatch',72),('after-acceptance',73),('before-readback',74)]:
  p=scenario('crash-'+name);c(p,'prepare','prepare',prep)
  if name not in ('index-only','durable-fence'):c(p,'mode','mode',name)
  c(p,'crash','crash-save',name,code);c(p,'status-after-crash','status');c(p,'second-process-save','save');c(p,'reconcile','reconcile');c(p,'status-final','status')
 p=scenario('state-matrix');c(p,'matrix','state-matrix');c(p,'old-reader','status')
 p=scenario('admission');c(p,'consume-original','approval')
 p=scenario('cism-legacy');c(p,'seed','cism','seed-legacy');c(p,'blocked-new-request','cism','s10-cism-first');c(p,'legacy-read-uncertain','cism','get');c(p,'authoritative-external-new','edit-source',{'cismValue':'Legacy supported S10 narrative.'});c(p,'legacy-read-settle','cism','get');c(p,'separately-reviewed-completion','cism','s10-cism-completion');c(p,'old-reader-after-completion','cism','get')
 p=scenario('cism-operation');c(p,'first','cism','s10-cism-first');c(p,'completion','cism','s10-cism-completion');c(p,'read','cism','get')
 p=scenario('cism-operation-uncertain');c(p,'inject','edit-source',{'cismFail':True});c(p,'first','cism','s10-cism-first');c(p,'new-request-blocked','cism','s10-cism-next');c(p,'read','cism','get')
 p=scenario('workflow-lock');holder=launch(p,'lock',json.dumps({'hold':5000,'marker':'holder'}));start=time.monotonic()
 try:
  while not (p/'holder').exists():
   if holder.poll() is not None or time.monotonic()-start>3:raise RuntimeError('s10_holder_not_ready')
   time.sleep(.01)
  first=load_lock(p);c(p,'second-process-deadline','lock',{'waitMs':140,'marker':'waiter'});
  if (p/'waiter').exists():raise RuntimeError('s10_simultaneous_workflow_owner')
  os.killpg(holder.pid,signal.SIGKILL);f=collect(holder,p,'killed-holder',expected=-signal.SIGKILL);transcript.append({'scenario':p.name,**f})
  c(p,'acquire-after-kill','lock',{'waitMs':500,'marker':'recovered'});second=load_lock(p)
  if first!=second:raise RuntimeError('s10_lock_inode_replaced')
  transcript.append({'scenario':p.name,'sameKernelObject':True,'killedHolderReaped':True,'deadlineRefused':True})
 finally:
  if holder.poll() is None:os.killpg(holder.pid,signal.SIGKILL);holder.communicate(timeout=3)
 dump(root/'transcript.json',transcript)
 return transcript
def load_lock(p):
 files=list((p/'store'/'locks').glob('*.flock'))
 if len(files)!=1:raise RuntimeError('s10_one_workflow_lock_required')
 s=files[0].stat();return s.st_dev,s.st_ino
def main():
 a=argparse.ArgumentParser();a.add_argument('--root',required=True);a.add_argument('--freeze',action='store_true');a.add_argument('--emit',action='store_true');args=a.parse_args();root=pathlib.Path(args.root).absolute()
 root.mkdir(mode=0o700,parents=True,exist_ok=False)
 actual=run(root);expected=HERE/'EXPECTED-v2.json'
 if args.freeze:
  if expected.exists():raise RuntimeError('s10_frozen_expected_exists')
  dump(expected,actual)
 else:
  from judge import canonical
  if canonical(actual)!=canonical(load(expected)):
   dump(root/'comparison.json',{'equal':False});raise RuntimeError('s10_frozen_transcript_mismatch')
 dump(root/'RESULT.json',{'status':'reference_observed' if args.freeze else 'match','frames':len(actual),'expectedSha256':hashlib.sha256(expected.read_bytes()).hexdigest(),'allChildrenReaped':True,'liveTransport':0,'compiler':False})
 print(json.dumps(actual,ensure_ascii=False,separators=(',',':')) if args.emit else json.dumps(load(root/'RESULT.json')))
if __name__=='__main__':main()
