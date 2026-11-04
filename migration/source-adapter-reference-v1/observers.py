import pathlib,argparse,json,hashlib
from run import execute
p=argparse.ArgumentParser();p.add_argument('--reference-root',required=True);p.add_argument('--source-root',required=True);p.add_argument('--output',required=True);p.add_argument('--freeze',action='store_true');a=p.parse_args();r=pathlib.Path(a.reference_root);out=pathlib.Path(a.output)
s=execute(['/usr/local/bin/bun','--no-env-file',str(r/'observer.ts'),a.source_root,a.reference_root,'@SCRATCH@'],out,timeout=8)
if s['exitCode']!=0 or s['signal'] or s['termination'] or s['remainingGroupMembers'] or (out/'stderr.bin').read_bytes():raise SystemExit('observer_source_run_failed')
files={'OBSERVERS-v2-EXPECTED.json':out/'stdout.bin','OBSERVERS-v2-EFFECTS.ndjson':out/'scratch/effects.ndjson'}
for name,path in files.items():
 if a.freeze:
  if (r/name).exists():raise SystemExit('observer_frozen_file_exists')
  (r/name).write_bytes(path.read_bytes())
 elif (r/name).read_bytes()!=path.read_bytes():raise SystemExit('observer_mismatch:'+name)
report={'pass':True,'frames':3,'hashes':{n:hashlib.sha256(p.read_bytes()).hexdigest() for n,p in files.items()},'status':s}
(out/'OBSERVERS.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
