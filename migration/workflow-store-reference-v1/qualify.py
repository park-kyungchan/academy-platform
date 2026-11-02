#!/usr/bin/env python3
import argparse,json,pathlib,sys
from judge import judge,HERE
a=argparse.ArgumentParser();a.add_argument('--root',required=True);args=a.parse_args();root=pathlib.Path(args.root).absolute();root.mkdir(parents=True,mode=0o700,exist_ok=False)
results=[]
for mode in ['wrong-value','missing-field','changed-identity','changed-effect','changed-refusal','bool-integer','nonzero','junk-stdout','stderr','denied-dispatch','fake-dispatch','extra-field','oversized-stdout','timeout','identity-control']:
 r=judge([sys.executable,str(HERE/'mutant.py'),mode],root/mode,deadline=.5 if mode=='timeout' else 30);results.append({'mode':mode,**r})
 if r['accepted']!=(mode=='identity-control'):raise RuntimeError('s10_mutant_judge_unqualified:'+mode)
(root/'RESULT.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps({'rejected':14,'positiveControl':1,'allChildrenReaped':True}))
