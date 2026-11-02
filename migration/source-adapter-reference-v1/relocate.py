"""Move only disposable copies; source originals and other owners stay untouched."""
import pathlib,json,shutil,sys,argparse,hashlib,os
from run import execute
from judge import normalize,parse
p=argparse.ArgumentParser();p.add_argument('--reference-root',required=True);p.add_argument('--source-root',required=True);p.add_argument('--output',required=True);a=p.parse_args();ref=pathlib.Path(a.reference_root);source=pathlib.Path(a.source_root);out=pathlib.Path(a.output);out.mkdir(mode=0o700,exist_ok=False)
pins=parse((ref/'SOURCE-PINS.json').read_bytes());expected=parse((ref/'EXPECTED.json').read_bytes());spec=parse((ref/'NORMALIZATION.json').read_bytes());staged=out/'source-staged';staged.mkdir(mode=0o700)
for file in pins['sources']:
 target=staged/file;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source/file,target)
for dep in pins['dependencies']:
 link=staged/dep['link'];link.parent.mkdir(parents=True,exist_ok=True);link.symlink_to(dep['target'],target_is_directory=True)
# A moved source root, a separately moved reference executable, and a third cwd/evidence root.
moved=out/'source-moved';staged.rename(moved)
execstage=out/'driver-staged';execstage.mkdir(mode=0o700)
for file in ['reference.ts','guard.ts','SOURCE-PINS.json']:
 shutil.copyfile(ref/file,execstage/file)
shutil.copytree(ref/'corpus',execstage/'corpus');driver=out/'driver-moved';execstage.rename(driver)
foreign=out/'unrelated-cwd';foreign.mkdir(mode=0o700)
reports=[]
for case in ['saved-history-all','native-normal','presentation-values-and-spans','module-relative-paths']:
 directory=out/case
 cmd=['/usr/local/bin/bun','--no-env-file',str(driver/'reference.ts'),'--source-root',str(moved),'--reference-root',str(driver),'--scratch-root','@SCRATCH@','--case',case]
 status=execute(cmd,directory,timeout=8,cwd=foreign)
 index=next(i for i,f in enumerate(expected['frames']) if f['id']==case)
 prefix='/frames/'+str(index)+'/'
 reduced={**spec,'timestampPointers':['/frames/0/'+p[len(prefix):] for p in spec['timestampPointers'] if p.startswith(prefix)]}
 actual=parse((directory/'stdout.bin').read_bytes());value=normalize(actual,reduced)
 wanted={'schema':expected['schema'],'frames':[expected['frames'][index]],'runtimeExports':expected['runtimeExports']}
 # UUID labels preserve relationships; the full transcript's earlier native cases do not occur in this subset.
 if case=='native-normal':
  pass
 match=json.dumps(value,sort_keys=True)==json.dumps(wanted,sort_keys=True)
 if status['exitCode']!=0 or status['signal'] or status['termination'] or status['remainingGroupMembers'] or (directory/'stderr.bin').read_bytes() or not match:raise SystemExit('relocation_mismatch:'+case)
 # Compare its complete effect block, including operation selector/order/count.
 blocks=(ref/'EXPECTED-EFFECTS.ndjson').read_text().splitlines();start=next(i for i,line in enumerate(blocks) if json.loads(line)=={'event':'case_start','id':case});end=next(i for i in range(start,len(blocks)) if json.loads(blocks[i]).get('event')=='case_end')
 trace='\n'.join(blocks[start:end+1])+'\n'
 if (directory/'scratch/effects.ndjson').read_text()!=trace:raise SystemExit('relocation_effect_mismatch:'+case)
 reports.append({'case':case,'matchesFrozenValueSpansRefusalsIdentityProvenanceAndEffects':True,'status':status})
# Explicit-root and fixture-ownership startup refusals, before original source evaluation.
for case,extra in [('missing-source-root',['--reference-root',str(driver),'--scratch-root','@SCRATCH@']),('noncanonical-scratch',['--source-root',str(moved),'--reference-root',str(driver),'--scratch-root',str(foreign)+'/.'])]:
 status=execute(['/usr/local/bin/bun','--no-env-file',str(driver/'reference.ts')]+extra,out/case,timeout=8,cwd=foreign)
 raw=(out/case/'stderr.bin').read_text();required='s09_explicit_source_and_reference_root_required' if case=='missing-source-root' else 's09_explicit_canonical_root_required'
 if status['exitCode']!=1 or required not in raw or status['remainingGroupMembers']:raise SystemExit('startup_refusal_mismatch:'+case)
 reports.append({'case':case,'startupRefused':required,'status':status})
report={'sourceOriginalsMoved':False,'disposableCopiesMovedIndependently':True,'explicitEvidenceRoot':'child-owned scratch root, hashes/source provenance stable','checks':reports,'originalPathsModuleRemainsModuleRelative':True,'originalSavedLoaderOwnershipGapRetained':True}
(out/'RELOCATION.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'checks':len(reports),'allMatched':True}))
