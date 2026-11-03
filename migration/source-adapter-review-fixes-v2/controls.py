"""Wrong actual children must fail the unmodified new frozen-original comparison."""
import sys
sys.dont_write_bytecode=True
import argparse,pathlib,json,importlib.util,shlex
ROOT=pathlib.Path(__file__).resolve().parents[2];base=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('review_native',base/'native.py');native=importlib.util.module_from_spec(spec);spec.loader.exec_module(native)
p=argparse.ArgumentParser();p.add_argument('--binary',required=True);p.add_argument('--output',required=True);p.add_argument('--mode',required=True,choices=['value','span','missing','refusal','effect']);a=p.parse_args();output=pathlib.Path(a.output);output.mkdir(mode=0o700);results=[]
# One native90 budget per mode; never aggregate five ~20s modes under90.
for mode in [a.mode]:
 launcher=output/(mode+'-child');raw=output/(mode+'-raw');raw.mkdir(mode=0o700)
 launcher.write_text('#!/bin/sh\nexec '+shlex.join(['/usr/bin/python3','-B',str(base/'mutant.py'),mode,a.binary,str(raw)])+'\n');launcher.chmod(0o700)
 directory=output/mode
 status=native.executor.execute(['/usr/bin/python3','-B',str(base/'compare.py'),'--binary',str(launcher),'--scratch','@SCRATCH@'],directory,90,cwd=ROOT)
 assert status['exitCode']!=0 and status['signal'] is None and status['termination'] is None and not status['remainingGroupMembers'],mode
 if (directory/'scratch/DIFF.json').exists():assert json.loads((directory/'scratch/DIFF.json').read_text()),mode
 else:assert (directory/'stderr.bin').read_bytes(),mode
 results.append({'mode':mode,'rejected':True,'status':status})
(output/'CONTROLS.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps({'checks':len(results),'allRejected':True}))
