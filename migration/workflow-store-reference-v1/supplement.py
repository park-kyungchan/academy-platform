#!/usr/bin/env python3
import argparse,json,pathlib
from run import call,normalize,dump,load
from judge import canonical,HERE
def run(root):
 root.mkdir(mode=0o700,parents=True,exist_ok=False);out=[];pids={}
 def c(name,label,op,arg='s10-request'):
  p=root/'cases'/name;p.mkdir(mode=0o700,parents=True,exist_ok=True);f=call(p,label,op,arg if isinstance(arg,str) else json.dumps(arg));pids[load(p/(label+'.channel.json'))['pid']]='<child:'+name+':'+label+'>';out.append({'scenario':name,**normalize(f,pids)});return f['data']['result']
 prep={'requestRef':'s10-request','field':'progress','value':'- 지정 문제 풀이 진행.'};name='manual-resolution'
 c(name,'prepare','prepare',prep);c(name,'timeout','mode','timeout-old');c(name,'save','save')
 request={'requestRef':'s10-request','instruction':{'text':'Resolve only this fictional execution as observed not persisted.','sessionRef':'s10-resolution','receivedAt':'2040-01-04T12:00:00.000Z'}}
 resolution={'request':request,'executions':[{'requestRef':'s10-request','field':'progress'}]}
 c(name,'wrong-current-request','reconcile',{'resolution':{**resolution,'request':{**request,'requestRef':'other-request'}}})
 c(name,'wrong-execution','reconcile',{'resolution':{**resolution,'executions':[{'requestRef':'foreign-request','field':'progress'}]}})
 c(name,'explicit-resolution','reconcile',{'resolution':resolution});c(name,'same-request-no-replay','save')
 c(name,'normal-mode','mode','normal');c(name,'prepare-new-reviewed-request','prepare',{**prep,'requestRef':'s10-separate-instruction'});c(name,'new-reviewed-request','save','s10-separate-instruction')
 c('unknown-before','review','review-held');c('bindings','original-refusals','binding-refusals')
 dump(root/'transcript.json',out);return out
if __name__=='__main__':
 a=argparse.ArgumentParser();a.add_argument('--root',required=True);a.add_argument('--freeze',action='store_true');args=a.parse_args();root=pathlib.Path(args.root).absolute();actual=run(root);expected=HERE/'SUPPLEMENT-EXPECTED.json'
 if args.freeze:
  if expected.exists():raise RuntimeError('s10_supplement_expected_exists')
  dump(expected,actual)
 elif canonical(actual)!=canonical(load(expected)):raise RuntimeError('s10_supplement_mismatch')
 dump(root/'RESULT.json',{'frames':len(actual),'match':not args.freeze,'allChildrenReaped':True});print(json.dumps({'frames':len(actual),'allChildrenReaped':True}))
