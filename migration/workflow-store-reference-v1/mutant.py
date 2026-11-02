#!/usr/bin/env python3
import json,os,pathlib,sys,time
HERE=pathlib.Path(__file__).resolve().parent
x=json.loads((HERE/'EXPECTED-v2.json').read_text());mode=sys.argv[1];exitcode=0
saved=next(f for f in x if f.get('label')=='save-first')['data']['result']
held=next(f for f in x if f.get('label')=='new-request-blocked')['data']['result']
if mode=='wrong-value':saved['academyWrites']=8
elif mode=='missing-field':del saved['parentTransmission']
elif mode=='changed-identity':saved['userContext']['scope']['studentId']='77002'
elif mode=='changed-effect':next(f for f in x if f.get('label')=='save-first')['data']['snapshot']['events'][-1]['effectRecordKey']='99'
elif mode=='changed-refusal':held['reasonCode']='duplicate_execution_already_settled'
elif mode=='bool-integer':saved['academyWrites']=True
elif mode=='nonzero':exitcode=17
elif mode=='junk-stdout':print('plausible debug output')
elif mode=='stderr':print('synthetic diagnostic',file=sys.stderr)
elif mode=='denied-dispatch':(pathlib.Path(os.environ['S10_MUTANT_EFFECT_ROOT'])/'denied.log').write_text('denied_transport\n')
elif mode=='fake-dispatch':(pathlib.Path(os.environ['S10_MUTANT_EFFECT_ROOT'])/'fake-effects.jsonl').write_text('{"operation":"fake_dispatch","effectRecordKey":"13"}\n')
elif mode=='extra-field':saved['inventedSuccess']=True
elif mode=='oversized-stdout':sys.stdout.write('x'*(5*1024*1024));sys.stdout.flush()
elif mode=='timeout':time.sleep(10)
elif mode!='identity-control':raise RuntimeError('unknown_mutant')
print(json.dumps(x,ensure_ascii=False,separators=(',',':')));raise SystemExit(exitcode)
