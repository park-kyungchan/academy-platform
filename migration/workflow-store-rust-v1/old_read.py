#!/usr/bin/env python3
"""Rust reads frozen original states; unchanged original TS reads actual Rust states."""
import argparse,base64,hashlib,json,os,pathlib,shutil,sys,ctypes
HERE=pathlib.Path(__file__).resolve().parent;REF=HERE.parent/'workflow-store-reference-v1'
sys.dont_write_bytecode=True
sys.path.insert(0,str(REF));import run as original
ORIGINAL_LAUNCH=original.launch
sys.path.insert(0,str(HERE));import verify
from fixture import load,put,js
from judge import canonical
import importlib.util
spec=importlib.util.spec_from_file_location('original_old',REF/'old_read.py');old=importlib.util.module_from_spec(spec);spec.loader.exec_module(old)
def hashes(root):return {str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in root.rglob('*') if p.is_file() and p.relative_to(root).parts[0] in ['store','cism']}
def main():
 a=argparse.ArgumentParser();a.add_argument('--root',required=True,type=pathlib.Path);a.add_argument('--binary',required=True);a.add_argument('--main-results',required=True,type=pathlib.Path);args=a.parse_args()
 if ctypes.CDLL(None).prctl(36,1,0,0,0)!=0:raise RuntimeError('s10_subreaper_unavailable')
 verify.RUST_BINARY=str(pathlib.Path(args.binary).resolve());args.root.mkdir(mode=0o700,parents=True,exist_ok=False)
 old.call=original.call;old.normalize=verify.normalize
 actual=old.run(args.root/'original-states');expected=load(REF/'OLD-EXPECTED.json')
 diffs=verify.differences(actual,expected);put(args.root/'original-comparison.json',{'frames':len(actual),'match':not diffs,'differences':diffs})
 if diffs:raise RuntimeError('s10_original_old_reader_mismatch')
 results=[]
 for name,scenario,label in old.SELECTION:
  frame=json.loads(load(args.main_results/'cases'/scenario/(label+'.channel.json'))['stdout']);snap=frame['snapshot'];dest=args.root/'rollback'/'cases'/name;dest.mkdir(mode=0o700,parents=True)
  for f in snap['files']:
   p=dest/'store'/f['path'];p.parent.mkdir(mode=0o700,parents=True,exist_ok=True);p.write_bytes(base64.b64decode(f['bytes']));p.chmod(0o600)
  if 'cism' in name:
   for f in frame['result']['files']:
    p=dest/'cism'/f['path'];p.parent.mkdir(mode=0o700,parents=True,exist_ok=True);p.write_bytes(base64.b64decode(f['bytes']));p.chmod(0o600)
  put(dest/'source.json',snap['source']);(dest/'events.jsonl').write_text(''.join(js(e)+'\n' for e in snap['events']));(dest/'events.jsonl').chmod(0o600)
  before=hashes(dest);op='cism' if 'cism' in name else 'status';arg='get' if op=='cism' else 's10-request'
  rust=original.call(dest,'rust-read-actual-rust-state',op,arg)
  original.launch=ORIGINAL_LAUNCH
  try:ts=original.call(dest,'unchanged-ts-rollback-reader',op,arg)
  finally:original.launch=verify.launch
  if rust['data']!=ts['data']:put(args.root/(name+'.difference.json'),verify.differences(rust['data'],ts['data']));raise RuntimeError('s10_mixed_reader_mismatch:'+name)
  after=hashes(dest)
  if any(after.get(k)!=v for k,v in before.items()):raise RuntimeError('s10_rollback_changed_retained_bytes:'+name)
  results.append({'fixture':name,'readerDataEqual':True,'retainedFiles':len(before),'retainedBytesUnchanged':True,'groupsGone':True})
 put(args.root/'RESULT.json',{'originalFrozenStates':len(actual),'originalExpectedMatch':True,'rustStatesThroughUnchangedTS':results,'allChildrenReaped':True});print(json.dumps({'originalFrozenStates':len(actual),'rustStatesThroughUnchangedTS':len(results),'retainedBytesUnchanged':True}))
if __name__=='__main__':main()
