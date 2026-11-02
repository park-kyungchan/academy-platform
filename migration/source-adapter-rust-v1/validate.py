"""Read-only unchanged-oracle comparison, with bounded path-level mismatch diagnostics."""
import sys
sys.dont_write_bytecode=True
import json,pathlib,argparse
from native import ROOT,module
REF=ROOT/'migration/source-adapter-reference-v1'
def diff(a,b,path='',out=None):
 out=[]if out is None else out
 if type(a)!=type(b):out.append([path,'type',repr(a)[:120],repr(b)[:120]])
 elif isinstance(a,dict):
  for k in sorted(set(a)|set(b)):
   if k not in a or k not in b:out.append([path+'/'+k,'presence',repr(a.get(k))[:120],repr(b.get(k))[:120]])
   else:diff(a[k],b[k],path+'/'+k,out)
 elif isinstance(a,list):
  if len(a)!=len(b):out.append([path,'length',len(a),len(b)])
  for i,(x,y)in enumerate(zip(a,b)):diff(x,y,path+'/'+str(i),out)
 elif a!=b:out.append([path,'value',repr(a)[:220],repr(b)[:220]])
 return out
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('directory');p.add_argument('--additive',action='store_true');a=p.parse_args();d=pathlib.Path(a.directory);judge=module(REF/'judge.py','immutable_s09_judge');actual=judge.parse((d/'stdout.bin').read_bytes())
 if a.additive:
  expected=judge.parse((REF/'OBSERVERS-v2-EXPECTED.json').read_bytes());differences=diff(actual,expected);status=json.loads((d/'RESULT.json').read_text());problems=[]
  if status['exitCode']!=0 or status['signal']or status['termination']or status['remainingGroupMembers']:problems.append('process_status')
  if(d/'stderr.bin').read_bytes():problems.append('stderr')
  if(d/'scratch/effects.ndjson').read_bytes()!=(REF/'OBSERVERS-v2-EFFECTS.ndjson').read_bytes():problems.append('persisted_effect_trace')
  if differences:problems.append('semantic_values')
  result={'pass':not problems,'problems':problems}
 else:
  result=judge.compare(d,REF);actual=judge.normalize(actual,judge.parse((REF/'NORMALIZATION.json').read_bytes()));expected=judge.parse((REF/'EXPECTED.json').read_bytes());differences=diff(actual,expected)
 (d/'COMPARISON.json').write_text(json.dumps({'judge':result,'differences':differences},ensure_ascii=True,indent=2)+'\n');print(json.dumps({'judge':result,'mismatches':len(differences),'first':differences[:10]},ensure_ascii=True));sys.exit(0 if result['pass']else 1)
