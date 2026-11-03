"""Independent readback of raw OS traces and frozen original semantic outputs."""
import sys
sys.dont_write_bytecode=True
import pathlib,json,re,hashlib,argparse,importlib.util,datetime
base=pathlib.Path(__file__).resolve().parent
p=argparse.ArgumentParser();p.add_argument('--original',required=True);p.add_argument('--rust',required=True);a=p.parse_args();original=pathlib.Path(a.original);rust=pathlib.Path(a.rust)
expected=json.loads((base/'ORIGINAL-EXPECTED.json').read_text())
for name in ['REGEXP','AST','UNICODE','BUDGET','CONTAINERS']:expected['presentation']+=json.loads((base/f'{name}-EXPECTED.json').read_text())['presentation']
observed=json.loads((original/'stdout.bin').read_bytes())
spec=importlib.util.spec_from_file_location('immutable_original_judge',base.parent/'source-adapter-reference-v1/judge.py');judge=importlib.util.module_from_spec(spec);spec.loader.exec_module(judge)
normalization=json.loads((base/'NEW-NORMALIZATION.json').read_text())
assert normalization['timestampPointers']==['/selector/0/result/value/observedAt']
for value,directory in [(observed,original),(expected,original.parent/'original-02')]:
 stamp=datetime.datetime.fromisoformat(value['selector'][0]['result']['value']['observedAt'].replace('Z','+00:00'));status=json.loads((directory/'RESULT.json').read_text())
 assert datetime.datetime.fromisoformat(status['startedAt'])<=stamp<=datetime.datetime.fromisoformat(status['finishedAt'])
assert judge.normalize(observed,normalization)==judge.normalize(expected,normalization)
calls=json.loads((rust/'scratch/RUST-CHILDREN.json').read_text());product_pids={c['pid']for c in calls};assert len(calls)==222
results={}
for label,directory in [('original',original),('rust',rust)]:
 status=json.loads((directory/'RESULT.json').read_text());assert status['exitCode']==0 and not status['signal']and not status['termination']and not status['remainingGroupMembers'];assert not(directory/'stderr.bin').read_bytes()
 files=sorted((directory/'scratch').glob('syscalls.*'));assert files
 writes=[];network=[];execs=[];products=[];pins={}
 for f in files:
  pid=int(f.suffix[1:]);raw=f.read_bytes();text=raw.decode();pins[f.name]=hashlib.sha256(raw).hexdigest()
  selected=label=='original'or pid in product_pids
  if not selected:continue
  products.append(pid)
  for line in text.splitlines():
   if 'execve('in line:execs.append({'pid':pid,'line':line});assert ('/usr/local/bin/bun"'in line if label=='original'else calls[0]['argv'][0]+'"'in line)
   if re.search(r'\b(?:socket|socketpair|connect|bind|listen|accept|sendto|sendmsg|recvfrom|recvmsg)\(',line):network.append({'pid':pid,'line':line});assert label=='original'and 'AF_INET'not in line
   if re.search(r'\bopen(?:at|at2)?\(',line)and re.search(r'O_(?:WRONLY|RDWR|CREAT|TRUNC|APPEND)',line):
    writes.append({'pid':pid,'line':line});assert str(directory/'scratch/effects.ndjson')in line,line
   if re.search(r'\b(?:rename|unlink|mkdir|rmdir|link|symlink|chmod|chown|truncate)(?:at|at2)?\(',line):raise AssertionError(line)
 assert len(execs)==(1 if label=='original'else len(calls))
 results[label]={'tracePins':pins,'selectedPids':products,'execs':execs,'networkCalls':network,'writeOpens':writes,'status':status}
assert json.loads((rust/'stdout.bin').read_bytes())=={'presentation':202,'redirects':12,'admission':8,'differences':0,'first':[],'pass':True}
print(json.dumps({'pass':True,'actualOriginalMatchesAllFrozenExpectations':True,'rustChildren':len(calls),'traces':results},sort_keys=True))
