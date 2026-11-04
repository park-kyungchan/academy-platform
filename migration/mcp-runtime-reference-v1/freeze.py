#!/usr/bin/env python3
"""One-shot original-output freeze. Refuses existing destinations; no Rust input."""
import argparse,json,pathlib,hashlib,shutil,datetime
from judge import normalize
HERE=pathlib.Path(__file__).resolve().parent
def digest(b):return hashlib.sha256(b).hexdigest()
def main():
 a=argparse.ArgumentParser();a.add_argument('--native-root',required=True,type=pathlib.Path);x=a.parse_args()
 target=HERE/'frozen';target.mkdir(exist_ok=False)
 manifest={'schema':'s11.independent-original-freeze/1','frozenAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),'beforeAnyRustCandidate':True,'modes':{},'files':{}}
 for mode in ['inventory','discovery','protocol','composition','graph','identity','framing','bounds','launcher','relocation']:
  src=x.native_root/('frozen-'+mode);dest=target/mode;dest.mkdir()
  if not (src/'COMPLETE.json').exists() or (src/'FAILED.json').exists():raise RuntimeError('original_mode_not_completed:'+mode)
  before=json.loads((src/'PINS-before.json').read_text());after=json.loads((src/'PINS-after.json').read_text())
  if before!=after:raise RuntimeError('source_or_installed_dependency_changed:'+mode)
  for name in ['observations.json','COMPLETE.json','OBSERVER-PINS.json','SOURCE-MIRROR.json','registry.json']:
   if (src/name).exists():shutil.copyfile(src/name,dest/name)
  raw=json.loads((src/'observations.json').read_text());(dest/'expected.json').write_text(json.dumps(normalize(raw,src),ensure_ascii=False,indent=2)+'\n')
  for pattern in ['commands/*/START.json','commands/*/RESULT.json','commands/*/stdin.bin','commands/*/stdout.bin','commands/*/stderr.bin','processes/*/*']:
   for p in src.glob(pattern):
    if p.is_file():q=dest/p.relative_to(src);q.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,q)
  # No emitted/read bytes means an empty channel; recorder opens files on first
  # data/write. This is explicit zero-byte observation, not missing transcript.
  for d in (dest/'processes').iterdir():
   start=json.loads((d/'START.json').read_text());origins={}
   # The tracer itself emits no stdout. Its attached child is the only writer
   # of these pipes; use that boundary to include Bun-native console/errors
   # that bypass the JS stream.write tap.
   for command in (dest/'commands').iterdir():
    command_start=json.loads((command/'START.json').read_text())
    if start['argv'][1] in command_start['originalArgv']:
     for name in ['stdin.bin','stdout.bin','stderr.bin']:
      shutil.copyfile(command/name,d/name);origins[name]='direct_original_child_pipe:'+str(command.relative_to(dest))
   for name in ['stdin.bin','stdout.bin','stderr.bin']:
    if not (d/name).exists():(d/name).write_bytes(b'');origins[name]='no_stream_data_or_write_observed'
    elif name not in origins:origins[name]='inner_original_stream_emit_or_write_tap'
   (d/'CHANNEL-ORIGINS.json').write_text(json.dumps(origins,indent=2)+'\n')
  manifest['modes'][mode]={'nativeRoot':str(src),'observations':len(raw),'originalPinsSha256':digest((src/'PINS-before.json').read_bytes()),'processes':len(list((dest/'processes').iterdir())),'note':'full original file/process/network syscall traces and effect files remain at nativeRoot; all raw Bun channels copied here'}
 for p in sorted(target.rglob('*')):
  if p.is_file():manifest['files'][str(p.relative_to(HERE))]={'bytes':p.stat().st_size,'sha256':digest(p.read_bytes())}
 for name in ['PINS-before-v2.json','GRAPH-v2.json','observe.py','pins.py','preload.ts','fixture.ts','judge.py','freeze.py']:
  p=HERE/name;manifest['files'][name]={'bytes':p.stat().st_size,'sha256':digest(p.read_bytes())}
 p=HERE/'FROZEN-MANIFEST.json'
 with p.open('x') as f:json.dump(manifest,f,indent=2);f.write('\n')
 print(json.dumps({'modes':len(manifest['modes']),'observations':sum(v['observations'] for v in manifest['modes'].values()),'files':len(manifest['files']),'frozenAt':manifest['frozenAt']}))
if __name__=='__main__':main()
