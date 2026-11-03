#!/usr/bin/env python3
"""Strict reference integrity and transcript comparison. No output generation.

Object member order is immaterial, arrays/omission/types/names/schemas are not.
Identity hashes and failure codes are deliberately NOT ignored.
"""
import argparse,copy,hashlib,json,pathlib,re,sys
HERE=pathlib.Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
canonical=lambda v:json.dumps(v,ensure_ascii=False,sort_keys=True,separators=(',',':'),allow_nan=False)
UUID=re.compile(r'\b[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\b')
CLOCK_KEYS={'observedAt','completedAt','issuedAt','processStartedAt','at','expiresAt'}
def normalize(value,mode_root):
 uuids={};clocks={}
 def walk(v,key=''):
  if key=='elapsedMs':
   if type(v) is not int or v<0:raise ValueError('elapsed_ms_type_or_bound_changed')
   return '@elapsedMs@'
  if isinstance(v,dict):return {k:walk(x,k) for k,x in v.items()}
  if isinstance(v,list):return [walk(x) for x in v]
  if isinstance(v,str):
   # JSON dual representations remain separately present. Re-encode in the
   # original member order, so text serialization reorder is still detected.
   if key=='text':
    try:parsed=json.loads(v)
    except ValueError:pass
    else:return json.dumps(walk(parsed),ensure_ascii=False,separators=(',',':'))
   if key in CLOCK_KEYS and re.fullmatch(r'2026-11-03T\d\d:\d\d:\d\d(?:\.\d+)?Z',v):
    if v not in clocks:clocks[v]='@clock:'+str(len(clocks)+1)+'@'
    return clocks[v]
   def uuid(m):
    s=m.group()
    if s not in uuids:uuids[s]='@uuid:'+str(len(uuids)+1)+'@'
    return uuids[s]
   return UUID.sub(uuid,v.replace(str(mode_root),'@MODE_ROOT@'))
  return v
 return walk(value)
def differences(a,b,path='$',limit=20):
 out=[]
 def visit(x,y,p):
  if len(out)>=limit:return
  if type(x)!=type(y):out.append({'path':p,'change':'type','expected':type(x).__name__,'actual':type(y).__name__});return
  if isinstance(x,dict):
   for k in x.keys()-y.keys():out.append({'path':p+'.'+k,'change':'omitted'})
   for k in y.keys()-x.keys():out.append({'path':p+'.'+k,'change':'added'})
   for k in sorted(x.keys()&y.keys()):visit(x[k],y[k],p+'.'+k)
  elif isinstance(x,list):
   if len(x)!=len(y):out.append({'path':p,'change':'array_length','expected':len(x),'actual':len(y)})
   for i,(u,v) in enumerate(zip(x,y)):visit(u,v,p+'['+str(i)+']')
  elif x!=y:out.append({'path':p,'change':'value','expectedSha256':sha(canonical(x).encode()),'actualSha256':sha(canonical(y).encode())})
 visit(a,b,path);return out[:limit]
def child_success(outer,children,source_before,source_after):
 return outer==0 and all(type(x) is int and x==0 for x in children) and source_before==source_after
def candidate_graph_qualified(v):
 # Qualification is a contract predicate, never inferred from status/count.
 return v.get('graphQualification')=='artifact_pinned' and v.get('completeGraphVerified') is True and v.get('mandatoryMismatch') is False and v.get('nativeOwnersRetained') is True
def verify_reference(source_root):
 manifest=json.loads((HERE/'FROZEN-MANIFEST.json').read_text());errors=[]
 for relative,d in manifest['files'].items():
  p=HERE/relative
  if not p.is_file() or sha(p.read_bytes())!=d['sha256']:errors.append('artifact_changed:'+relative)
 pins=json.loads((HERE/'PINS-before-v2.json').read_text())
 for relative,d in pins['sources'].items():
  p=source_root/relative
  if not p.is_file() or sha(p.read_bytes())!=d['sha256']:errors.append('original_source_changed:'+relative)
 for root,package in pins['packages'].items():
  for relative,d in package['files'].items():
   p=pathlib.Path(root)/relative
   if not p.is_file() or sha(p.read_bytes())!=d['sha256']:errors.append('installed_dependency_changed:'+str(p))
 for mode in manifest['modes']:
  p=HERE/'frozen'/mode
  raw=json.loads((p/'observations.json').read_text());expected=json.loads((p/'expected.json').read_text())
  delta=differences(expected,normalize(raw,manifest['modes'][mode]['nativeRoot']))
  if delta:errors.append({'mode':mode,'expected_not_from_raw':delta})
 return errors
def controls():
 p=HERE/'frozen/inventory/expected.json';base=json.loads(p.read_text());result=[]
 def check(name,mutator):
  v=copy.deepcopy(base);mutator(v);d=differences(base,v)
  if not d:raise RuntimeError('negative_control_not_detected:'+name)
  result.append({'id':name,'detected':True,'firstDifference':d[0]})
 tool=lambda v:v[0]['response']['result']['tools'][1]
 check('EDU-21-same-count-renamed',lambda v:tool(v).update(name='fictional_renamed_tool'))
 check('EDU-22-schema',lambda v:tool(v)['inputSchema'].update(type='array'))
 check('EDU-22-annotation',lambda v:tool(v)['annotations'].update(readOnlyHint=False))
 check('EDU-22-omission',lambda v:tool(v).pop('description'))
 # Probe bounds/defaults are real SDK-normalized declarations in original output.
 b=json.loads((HERE/'frozen/bounds/expected.json').read_text())
 tools=b[0]['response']['result']['tools'];probe=next(t for t in tools if t['name']=='academy_bounded_probe')
 for field,value in [('maximum',4),('default',3)]:
  v=copy.deepcopy(probe);v['inputSchema']['properties']['limit'][field]=value;d=differences(probe,v)
  if not d:raise RuntimeError('bound_or_default_drift_missed')
  result.append({'id':'EDU-22-'+field,'detected':True,'firstDifference':d[0]})
 # Exact names alone cannot mask endpoint/upstream changes.
 graph=json.loads((HERE/'GRAPH-v2.json').read_text());routes=graph['selectedOriginalRoutes'];v=copy.deepcopy(routes);v[0]['endpoint']='saved'
 if not differences(routes,v):raise RuntimeError('route_drift_missed')
 result.append({'id':'same-names-routing-drift','detected':True})
 if candidate_graph_qualified({'status':'observed','graphQualification':'legacy_graph_incomplete','completeGraphVerified':True,'mandatoryMismatch':False,'nativeOwnersRetained':True}):raise RuntimeError('unqualified_graph_accepted')
 result.append({'id':'EDU-23-unqualified-graph','detected':True})
 if child_success(0,[23],'a','a') or child_success(0,[0],'a','b'):raise RuntimeError('lying_wrapper_or_changed_source_accepted')
 result.extend([{'id':'BUILD-01-lying-wrapper','detected':True},{'id':'BUILD-01-source-after-green','detected':True}]);return result
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--source-root',type=pathlib.Path);p.add_argument('--controls',action='store_true');p.add_argument('--compare',choices=['inventory','discovery','protocol','composition','graph','identity','framing','bounds','launcher','relocation']);p.add_argument('--actual',type=pathlib.Path);p.add_argument('--mode-root',type=pathlib.Path);a=p.parse_args()
 if a.controls:print(json.dumps(controls(),indent=2))
 elif a.compare:
  expected=json.loads((HERE/'frozen'/a.compare/'expected.json').read_text());actual=normalize(json.loads(a.actual.read_text()),a.mode_root)
  errors=differences(expected,actual);print(json.dumps({'differences':errors,'exactWithinDeclaredNormalization':not errors},indent=2));sys.exit(bool(errors))
 else:
  if not a.source_root:p.error('--source-root required')
  errors=verify_reference(a.source_root.resolve());print(json.dumps({'referenceIntegrity':not errors,'errors':errors},indent=2));sys.exit(bool(errors))
