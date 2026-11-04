import pathlib,argparse,json,fcntl,os,hashlib
from run import execute
from judge import compare
p=argparse.ArgumentParser();p.add_argument('--reference-root',required=True);p.add_argument('--source-root',required=True);p.add_argument('--baseline',required=True);p.add_argument('--output',required=True);a=p.parse_args();r=pathlib.Path(a.reference_root);out=pathlib.Path(a.output);out.mkdir(mode=0o700,exist_ok=False)
controls=[]
for mode in ['wrong-value','boolean-as-zero','broken-clock-relation','missing-field','changed-identity','changed-source-account','changed-effect','changed-refusal','changed-raw-span','plausible-nonzero','junk-stdout','junk-stderr','unintended-fake-dispatch','denied-live-attempt']:
 cmd=['/usr/local/bin/bun','--no-env-file',str(r/'denied-mutant.ts'),a.baseline,'@SCRATCH@'] if mode=='denied-live-attempt' else ['/usr/bin/python3',str(r/'mutant.py'),mode,a.reference_root,a.baseline,'@SCRATCH@']
 path=out/mode;status=execute(cmd,path,timeout=8);judged=compare(path,r);controls.append({'mode':mode,'rejected':not judged['pass'],'judge':judged,'status':status})
 if judged['pass']:raise SystemExit('required_mutant_survived:'+mode)
bounds=[]
for mode in ['hung-descendant','output-flood','malformed-output','signal','stalled-source-body']:
 path=out/mode
 cmd=['/usr/local/bin/bun','--no-env-file',str(r/'reference.ts'),'--source-root',a.source_root,'--reference-root',a.reference_root,'--scratch-root','@SCRATCH@','--case','http-stalled-body'] if mode=='stalled-source-body' else ['/usr/bin/python3',str(r/'mutant.py'),mode,a.reference_root,a.baseline,'@SCRATCH@']
 status=execute(cmd,path,timeout=4 if mode=='stalled-source-body' else 1.2,output_limit=65536 if mode=='output-flood' else 1024*1024)
 facts={'mode':mode,'status':status,'lockReleased':None,'trace':(path/'scratch/effects.ndjson').read_text() if (path/'scratch/effects.ndjson').exists() else None}
 if mode=='hung-descendant':
  fd=os.open(path/'scratch/fixture.lock',os.O_RDWR)
  try:fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB);facts['lockReleased']=True;fcntl.flock(fd,fcntl.LOCK_UN)
  finally:os.close(fd)
  facts['descendantStart']=json.loads((path/'scratch/descendant-start.json').read_text());facts['descendantPidEvidenceExists']=(path/'scratch/descendant.pid').exists()
 if status['remainingGroupMembers']:raise SystemExit('descendant_cleanup_failed:'+mode)
 if mode in ['hung-descendant','stalled-source-body'] and (status['termination']!='deadline' or status['signal']!=9):raise SystemExit('deadline_not_observed:'+mode)
 if mode=='output-flood' and status['termination']!='output_limit':raise SystemExit('output_bound_not_observed')
 bounds.append(facts)
report={'mutants':controls,'bounds':bounds,'allRequiredMutantsRejected':all(x['rejected'] for x in controls),'allAuthoredGroupsEmpty':all(not x['status']['remainingGroupMembers'] for x in bounds+controls)}
(out/'QUALIFICATION.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'mutants':len(controls),'bounds':len(bounds),'allRejected':report['allRequiredMutantsRejected'],'groupsEmpty':report['allAuthoredGroupsEmpty']}))
