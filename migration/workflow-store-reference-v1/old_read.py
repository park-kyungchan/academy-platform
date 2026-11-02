#!/usr/bin/env python3
"""Replay immutable synthetic old bytes through unchanged ORIGINAL TS readers."""
import argparse,base64,hashlib,json,os,pathlib,shutil
from run import call,normalize,dump,load
HERE=pathlib.Path(__file__).resolve().parent
SELECTION=[('settled','duplicate','save-first'),('uncertain','uncertain-old','save-timeout'),('index-only','crash-index-only','status-after-crash'),('fenced','crash-durable-fence','status-after-crash'),('before-dispatch','crash-before-dispatch','status-after-crash'),('accepted-no-readback','crash-after-acceptance','status-after-crash'),('before-readback','crash-before-readback','status-after-crash'),('observed-persisted','uncertain-old','observe-desired'),('all-field-states','state-matrix','matrix'),('cism-legacy-unresolved','cism-legacy','seed'),('cism-legacy-settled','cism-legacy','legacy-read-settle'),('cism-legacy-and-operation','cism-legacy','separately-reviewed-completion'),('cism-operation-settled','cism-operation','completion'),('cism-operation-unresolved','cism-operation-uncertain','first')]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def freeze(raw):
 dest=HERE/'fixtures';dest.mkdir(exist_ok=False);manifest=[]
 for name,scenario,label in SELECTION:
  channel=load(raw/'cases'/scenario/(label+'.channel.json'));frame=json.loads(channel['stdout']);snap=frame['snapshot'];p=dest/name;p.mkdir()
  files=snap['files']+([{**f,'path':'cism/'+f['path']} for f in frame['result']['files']] if 'cism' in name else [])
  for f in files:
   if '/audit/' in '/'+f['path'] or f['path'].endswith('.flock'):continue
   target=p/('' if f['path'].startswith('cism/') else 'store')/f['path'];target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(base64.b64decode(f['bytes']));target.chmod(0o600)
  dump(p/'source.json',snap['source'])
  # Original event bytes provide action/consumption identity relationships.
  (p/'events.jsonl').write_text(''.join(json.dumps(e,ensure_ascii=False,separators=(',',':'))+'\n' for e in snap['events']))
  entries=[{'path':str(f.relative_to(p)),'sha256':digest(f),'bytes':f.stat().st_size} for f in sorted(p.rglob('*')) if f.is_file()]
  manifest.append({'name':name,'sourceScenario':scenario,'sourceLabel':label,'reader':'cism' if 'cism' in name else 'status','files':entries})
 dump(HERE/'OLD-STATES.json',manifest)
def run(root):
 root.mkdir(parents=True,exist_ok=False,mode=0o700);transcript=[]
 for item in load(HERE/'OLD-STATES.json'):
  name=item['name'];src=HERE/'fixtures'/name;dest=root/'cases'/name;dest.mkdir(parents=True,mode=0o700)
  for f in item['files']:
   source=src/f['path'];assert digest(source)==f['sha256'];p=dest/f['path'];p.parent.mkdir(parents=True,exist_ok=True,mode=0o700);shutil.copyfile(source,p);p.chmod(0o600)
  before={f['path']:digest(dest/f['path']) for f in item['files']}
  frame=call(dest,'old-original-reader',item['reader'],'get' if item['reader']=='cism' else 's10-request')
  for f,sha in before.items():
   if digest(dest/f)!=sha:raise RuntimeError('s10_old_bytes_changed:'+name+':'+f)
  transcript.append({'fixture':name,'oldBytesUnchanged':True,**normalize(frame)})
 dump(root/'transcript.json',transcript);return transcript
if __name__=='__main__':
 a=argparse.ArgumentParser();a.add_argument('--root');a.add_argument('--freeze-from');a.add_argument('--freeze-expected',action='store_true');args=a.parse_args()
 if args.freeze_from:freeze(pathlib.Path(args.freeze_from).absolute())
 else:
  actual=run(pathlib.Path(args.root).absolute());expected=HERE/'OLD-EXPECTED.json'
  if args.freeze_expected:
   if expected.exists():raise RuntimeError('s10_old_expected_exists')
   dump(expected,actual)
  else:
   from judge import canonical
   if canonical(actual)!=canonical(load(expected)):raise RuntimeError('s10_old_reader_mismatch')
  dump(pathlib.Path(args.root)/'RESULT.json',{'fixtures':len(actual),'oldBytesUnchanged':True,'allChildrenReaped':True,'expectedSha256':digest(expected)});print(json.dumps({'fixtures':len(actual),'oldBytesUnchanged':True}))
