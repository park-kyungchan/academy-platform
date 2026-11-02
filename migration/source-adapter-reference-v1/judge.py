"""Strict channels + normalized semantic values + exact original spans + effects judge."""
import json,pathlib,hashlib,datetime,re,sys,argparse
UUID=re.compile(r'^native-dayrecord-read:([a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12})(:current|:previous)$')

def parse(data):
 def pairs(items):
  out={}
  for k,v in items:
   if k in out:raise ValueError('duplicate_json_member')
   out[k]=v
  return out
 return json.loads(data,object_pairs_hook=pairs,parse_constant=lambda _:(_ for _ in ()).throw(ValueError('nonfinite_json')))

def normalize(value,spec):
 # These two timestamps are created from one source variable; preserve their equality.
 for frame in value.get('frames',[]):
  if frame.get('kind')=='native':
   for item in frame.get('value',{}).get('results',[]):
    if item.get('outcome')=='returned':
     observation=item['value']['observation']
     if any(t['observedAt']!=observation['observedAt'] for t in observation['toneSamples']):raise ValueError('native_clock_relationship_changed')
 timestamps=set(spec['timestampPointers']);seen=set();uuids={}
 def walk(x,p):
  if p in timestamps:
   if not isinstance(x,str) or not re.fullmatch(r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z',x):raise ValueError('source_timestamp_shape_changed@'+p)
   datetime.datetime.fromisoformat(x.replace('Z','+00:00'));seen.add(p);return '<SOURCE_CLOCK>'
  if isinstance(x,str):
   m=UUID.fullmatch(x)
   if m:
    key=m[1];uuids.setdefault(key,'reference-'+str(len(uuids)+1));return 'native-dayrecord-read:'+uuids[key]+m[2]
   return x
  if isinstance(x,list):return [walk(v,p+'/'+str(i)) for i,v in enumerate(x)]
  if isinstance(x,dict):return {k:walk(v,p+'/'+k.replace('~','~0').replace('/','~1')) for k,v in x.items()}
  return x
 out=walk(value,'')
 if seen!=timestamps:raise ValueError('required_timestamp_missing')
 return out

def compare(directory,ref):
 directory=pathlib.Path(directory);ref=pathlib.Path(ref);result=parse((directory/'RESULT.json').read_bytes());problems=[]
 if result['exitCode']!=0 or result['signal'] is not None or result['termination'] is not None:problems.append('process_status')
 if result['liveDescendantsRemaining'] or result['remainingGroupMembers']:problems.append('descendants_remaining')
 stdout=(directory/'stdout.bin').read_bytes();stderr=(directory/'stderr.bin').read_bytes()
 if stderr!=b'':problems.append('stderr')
 if not stdout.endswith(b'\n') or stdout.count(b'\n')!=1:problems.append('stdout_framing')
 try:
  actual=normalize(parse(stdout),parse((ref/'NORMALIZATION.json').read_bytes()));expected=parse((ref/'EXPECTED.json').read_bytes())
  if json.dumps(actual,sort_keys=True,ensure_ascii=True,separators=(',',':'))!=json.dumps(expected,sort_keys=True,ensure_ascii=True,separators=(',',':')):problems.append('required_value_presence_order_identity_span_refusal_or_effect')
 except Exception as e:problems.append('invalid_output:'+str(e)[:180])
 trace=directory/'scratch/effects.ndjson'
 if not trace.exists() or trace.read_bytes()!=(ref/'EXPECTED-EFFECTS.ndjson').read_bytes():problems.append('persisted_effect_trace')
 return {'pass':not problems,'problems':problems,'stdoutSha256':hashlib.sha256(stdout).hexdigest()}

if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--result',required=True);p.add_argument('--reference-root',required=True);a=p.parse_args();r=compare(a.result,a.reference_root);print(json.dumps(r));sys.exit(0 if r['pass'] else 1)
