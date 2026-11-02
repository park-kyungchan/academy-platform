"""One-time ORIGINAL source freeze. Never rewrites retained expectation files."""
import pathlib,json,hashlib,argparse
from judge import parse,normalize
p=argparse.ArgumentParser();p.add_argument('--result',required=True);p.add_argument('--reference-root',required=True);a=p.parse_args();run=pathlib.Path(a.result);ref=pathlib.Path(a.reference_root)
for name in ['EXPECTED.json','NORMALIZATION.json','EXPECTED-EFFECTS.ndjson','FREEZE.json']:
 if (ref/name).exists():raise SystemExit('s09_frozen_file_already_exists:'+name)
status=parse((run/'RESULT.json').read_bytes())
if status['exitCode']!=0 or status['signal'] or status['termination'] or (run/'stderr.bin').read_bytes() or status['remainingGroupMembers']:raise SystemExit('s09_original_run_not_clean')
raw=parse((run/'stdout.bin').read_bytes());paths=[]
def walk(x,p=''):
 if isinstance(x,dict):
  for k,v in x.items():
   key=p+'/'+k.replace('~','~0').replace('/','~1')
   # Only actual source-generated wall clocks, never dated fixture facts.
   if k in ['retrievedAt','observedAt'] and isinstance(v,str) and v.startswith('2026-11-02T'):paths.append(key)
   else:walk(v,key)
 elif isinstance(x,list):
  for i,v in enumerate(x):walk(v,p+'/'+str(i))
walk(raw)
spec={'schema':'s09.normalization/1','timestampPointers':paths,'timestampRule':'Validate exact millisecond UTC ISO shape, then replace only enumerated original-generated wall-clock leaves. Fixture dates/timestamps/raw source are unchanged. No sort/truncation/unknown-field deletion.','uuidRule':'Only native-dayrecord-read UUIDv4 evidence refs. Allocate stable labels in transcript traversal order; preserve uniqueness, equality, :current/:previous relations. No other identity normalized.','objectOrder':'JSON object member order is not compared; arrays, presence, numeric values, all strings/raw spans/cursor/digest bytes and ordered effect trace are exact.'}
(ref/'NORMALIZATION.json').write_text(json.dumps(spec,indent=2)+'\n')
(ref/'EXPECTED.json').write_text(json.dumps(normalize(raw,spec),ensure_ascii=False,indent=2)+'\n')
(ref/'EXPECTED-EFFECTS.ndjson').write_bytes((run/'scratch/effects.ndjson').read_bytes())
hashes={n:hashlib.sha256((ref/n).read_bytes()).hexdigest() for n in ['EXPECTED.json','EXPECTED-EFFECTS.ndjson','NORMALIZATION.json','corpus/CASES.json','SOURCE-PINS.json']}
(ref/'FREEZE.json').write_text(json.dumps({'schema':'s09.original-freeze/1','basis':'Original source run only, no Rust candidate executed/consulted','originalRunRawStdoutSha256':hashlib.sha256((run/'stdout.bin').read_bytes()).hexdigest(),'frames':len(raw['frames']),'hashes':hashes,'independentQualification':'pending Lead/Wonder'},indent=2)+'\n')
print(json.dumps({'frames':len(raw['frames']),'timestampLeaves':len(paths),'hashes':hashes}))
