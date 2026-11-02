"""Actual incorrect children must fail the unchanged original judge."""
import sys
sys.dont_write_bytecode=True
import pathlib,argparse,json
from native import ROOT,module,run
REF=ROOT/'migration/source-adapter-reference-v1';judge=module(REF/'judge.py','unchanged_judge')
p=argparse.ArgumentParser();p.add_argument('--output',required=True);p.add_argument('--baseline',required=True);a=p.parse_args();base=pathlib.Path(a.output);base.mkdir(mode=0o700);results=[]
for mode in ['wrong-value','missing-field','boolean-as-zero','changed-identity','changed-effect','changed-refusal','changed-raw-span','changed-source-account','broken-clock-relation','unintended-fake-dispatch','plausible-nonzero','junk-stdout','junk-stderr','malformed-output','signal']:
 output=base/mode;status=run(['/usr/bin/python3','-B',str(REF/'mutant.py'),mode,str(REF),a.baseline,'@SCRATCH@'],output,5);comparison=judge.compare(output,REF);assert not comparison['pass'],mode;results.append({'control':mode,'rejected':True,'problems':comparison['problems']})
(base/'CONTROL-RESULTS.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps({'checks':len(results),'allRejected':True}))
