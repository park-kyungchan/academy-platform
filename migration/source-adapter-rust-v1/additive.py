"""Three unchanged additive observers, all semantic values from actual Rust operations."""
import sys
sys.dont_write_bytecode=True
import argparse,json,pathlib
from compare import Driver,returned,ZERO,AUDIT,LESSON,PIN,js,sha
p=argparse.ArgumentParser();p.add_argument('--binary',required=True);p.add_argument('--scratch',required=True);a=p.parse_args();d=Driver(a.binary,a.scratch);frames=[]
def frame(name,fn):
 d.trace({'event':'case_start','id':name});result=fn();frames.append({'id':name,**result,'effects':ZERO});d.trace({'event':'case_end','id':name,'effects':ZERO})
def auth():
 v=d.native('auth-failure-twice',True)['value'];return returned({'first':v['results'][0],'second':v['results'][1],'authCalls':sum(e['operation']=='authenticate'for e in v['events'])})
def collision():
 results=[]
 for account in ['account-A','account-B']:
  v=d.native('observer',True)['value'];o=v['results'][0]['value']['observation'];results.append({'fixtureAccount':account,'sourceTarget':o['target'],'sourcePreviousTarget':o['previousTarget'],'sourceRevision':o['sourceRevision'],'wireDigest':o['wireDigest'],'authCalls':sum(e['operation']=='authenticate'for e in v['events'])})
 return returned({'results':results,'sourceDistinguishesAccounts':results[0]['sourceTarget']!=results[1]['sourceTarget'],'accountIsExternalFixtureMetadata':True})
def oversize():
 root=d.fixture('oversize',b'x'*(32*1024*1024+1),b'{}');return d.saved(root,{'query':{'kind':'roster'}})
frame('authentication-error-own-observers',auth);frame('same-principal-two-accounts-same-native-key',collision);frame('saved-loader-32MiB-leaf-refusal',oversize)
print(js({'schema':'s09.additional-source-observers/1','frames':frames}));(d.scratch/'RUST-CHILDREN.json').write_text(json.dumps(d.calls,indent=2)+'\n')
