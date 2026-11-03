"""Actual named binary comparisons with frozen originals, exact effects and raw children."""
import sys
sys.dont_write_bytecode=True
import pathlib,json,argparse
ROOT=pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'migration/source-adapter-rust-v1'))
from compare import Driver,REF,sha,js,primary,MAIN,SELECTED,PRINCIPAL
from validate import diff
import importlib.util
projection_spec=importlib.util.spec_from_file_location('original_span_projection',pathlib.Path(__file__).resolve().parent/'span_projection.py');projection=importlib.util.module_from_spec(projection_spec);projection_spec.loader.exec_module(projection)
p=argparse.ArgumentParser();p.add_argument('--binary',required=True);p.add_argument('--scratch',required=True);a=p.parse_args()
base=pathlib.Path(__file__).resolve().parent
frozen=json.loads((base/'FREEZE.json').read_text())
for path,h in frozen['files'].items():assert sha((base/path).read_bytes())==h,path
expected=json.loads((base/'ORIGINAL-EXPECTED.json').read_bytes());stimuli=json.loads((base/'STIMULI.json').read_bytes())
d=Driver(a.binary,a.scratch);results=[];differences=[]
regexp_input=json.loads((base/'REGEXP-STIMULI.json').read_bytes());regexp_expected=json.loads((base/'REGEXP-EXPECTED.json').read_bytes())
for n,h in json.loads((base/'REGEXP-FREEZE.json').read_text())['files'].items():assert sha((base/n).read_bytes())==h,n
ast_input=json.loads((base/'AST-STIMULI.json').read_bytes());ast_expected=json.loads((base/'AST-EXPECTED.json').read_bytes())
for n,h in json.loads((base/'AST-FREEZE.json').read_text())['files'].items():assert sha((base/n).read_bytes())==h,n
unicode_input=json.loads((base/'UNICODE-STIMULI.json').read_bytes());unicode_expected=json.loads((base/'UNICODE-EXPECTED.json').read_bytes())
for n,h in json.loads((base/'UNICODE-FREEZE.json').read_text())['files'].items():assert sha((base/n).read_bytes())==h,n
budget_input=json.loads((base/'BUDGET-STIMULI.json').read_bytes());budget_expected=json.loads((base/'BUDGET-EXPECTED.json').read_bytes())
for n,h in json.loads((base/'BUDGET-FREEZE.json').read_text())['files'].items():assert sha((base/n).read_bytes())==h,n
containers_input=json.loads((base/'CONTAINERS-STIMULI.json').read_bytes());containers_expected=json.loads((base/'CONTAINERS-EXPECTED.json').read_bytes())
for n,h in json.loads((base/'CONTAINERS-FREEZE.json').read_text())['files'].items():assert sha((base/n).read_bytes())==h,n
presentation_cases=stimuli['presentation']+regexp_input['presentation']+ast_input['presentation']+unicode_input['presentation']+budget_input['presentation']+containers_input['presentation'];presentation_expected=expected['presentation']+regexp_expected['presentation']+ast_expected['presentation']+unicode_expected['presentation']+budget_expected['presentation']+containers_expected['presentation']
for c,e in zip(presentation_cases,presentation_expected):
 assert all(v==0 or v is False for v in e['effects'].values());assert c['id']==e['id'];actual=d.call({k:v for k,v in c.items() if k!='id'});delta=diff(actual,projection.project(c,e['result']));differences += [[c['id'],*x]for x in delta];results.append({'id':c['id'],'result':actual})
for e in expected['redirects']:
 assert e['effects']['fakeDispatches']==len(e['calls']) and all(v==0 or v is False for k,v in e['effects'].items() if k!='fakeDispatches')
 records=[]
 for i,c in enumerate(e['calls']):
  record={'path':c['path'],'method':c['method'],'selector':c['selector'],'status':200,'body':'','headers':[]}
  if i==0:record.update(body=(REF/'corpus/login.html').read_text(),headers=[['Set-Cookie','JSESSIONID=redirect; Path=/']])
  elif i==1:record.update(status=302,headers=[['Location',e['location']],['Set-Cookie','JSESSIONID=follow; Path=/']],credentialProof={'userid':'s09-fictional-user','userpwd':'s09-fictional-password'})
  elif c['path']=='/servlet/controller.coursemanage.CourseManageServlet':record['body']=(REF/'corpus/principal.html').read_text()
  elif e['id']==11:record.update(status=302,headers=[['Location','/again']])
  records.append(record)
 before=d.effect.stat().st_size
 actual=d.call({'op':'http_session','principal':PRINCIPAL,'username':'s09-fictional-user','password':'s09-fictional-password','recordings':records,'commands':[{'method':'authenticate'}],'effectTrace':str(d.effect)})
 assert actual['outcome']=='returned'
 value=actual['value'];delta=diff(value['results'][0],e['result']);differences += [['redirect-'+str(e['id']),*x]for x in delta]
 assert value['authenticatedAfterClose'] is False
 assert value['authenticatedBeforeClose'] == (e['result']['outcome']=='returned')
 assert len(value['calls'])==len(e['calls'])
 for x,y in zip(value['calls'],e['calls']):
  assert (x['path'],x['method'],x['selector'],x['hasCookie'],x.get('query',[])) == (y['path'],y['method'],y['selector'],bool(y['cookie']),y['query'])
 events=[json.loads(x)for x in d.effect.read_bytes()[before:].splitlines()];assert all(x['event']=='recorded_dispatch' for x in events);assert len(events)==len(e['calls'])
 results.append({'id':'redirect-'+str(e['id']),'result':actual,'persistedEvents':events})
# Root-authorized shared fence: total counts selected by the actual original full-client injection.
for e in expected['selector']:
 assert all(v==0 or v is False for k,v in e['effects'].items() if k!='fakeDispatches')
 assert e['selectedDispatches']==(1 if e['extras']==1 else 0)
 before=d.effect.read_bytes()
 actual=d.call({'op':'operation_admission','origin':'https://dc.gang-a.kr','path':SELECTED,'method':'GET','query':[['reqCmd','SelSglRecord']],'body':[['reqCmd','SelSglRecord']]*e['extras']})
 assert d.effect.read_bytes()==before
 assert (actual['outcome']=='returned') == (e['result']['outcome']=='returned');results.append({'id':'total-selector-'+str(e['extras']),'result':actual,'persistedEffectBytesAdded':len(d.effect.read_bytes())-len(before)})
for selector in ['udtPrg','udtHw','udtMemo','udtAttn','SetDailyTest','SetHomeWorkRate']:
 before=d.effect.read_bytes()
 actual=d.call({'op':'operation_admission','origin':'https://dc.gang-a.kr','path':MAIN,'method':'GET','query':[['reqCmd',selector]],'body':[]});assert actual['outcome']=='threw';assert d.effect.read_bytes()==before;results.append({'id':'mutating-'+selector,'result':actual,'persistedEffectBytesAdded':len(d.effect.read_bytes())-len(before)})
(d.scratch/'ACTUAL.json').write_text(js(results)+'\n');(d.scratch/'DIFF.json').write_text(json.dumps(differences,ensure_ascii=True,indent=2)+'\n');(d.scratch/'RUST-CHILDREN.json').write_text(json.dumps(d.calls,indent=2)+'\n')
print(json.dumps({'presentation':len(presentation_cases),'redirects':len(expected['redirects']),'admission':8,'differences':len(differences),'first':differences[:10],'pass':not differences}));sys.exit(0 if not differences else 1)
