"""New original and Rust observers run independently on the same fictional source bytes."""
import sys
sys.dont_write_bytecode=True
import argparse,pathlib,json,datetime,re
from compare import Driver,REF,ROOT,sha,js,ZERO
from native import run
from validate import diff
p=argparse.ArgumentParser();p.add_argument('--binary',required=True);p.add_argument('--scratch',required=True);p.add_argument('--output',required=True);p.add_argument('--source-root',required=True);a=p.parse_args();d=Driver(a.binary,a.scratch)
html1="😀<table id=repair><tr><td>A&amp;B<td>C</table><select id=duplicate><option disabled>X<option> B  C </select><input id=duplicate value=second><textarea>\r\nA&amp;B\r\n</textarea><script id=handler>function onFoPrg(){return '가😀}';}</script>"
html2="가😀<table id=repair><tbody><tr><td><input value='&quot;😀'><td><textarea>\nfirst\r\nsecond</textarea></tr></tbody></table><select id=duplicate><optgroup disabled><option selected>A</optgroup><option>B</select><script id=handler>function onFoPrg(){/* } */return 'x';}</script>"
audit={'schemaVersion':'academy.audit.normalized/1.0.0','collectedAt':'2041-02-03T04:05:06.000Z','students':[{'studentId':x,'name':'Fictional '+str(i),'books':[],'sections':[]}for i,x in enumerate(['\ud800','0001','0001 ','0001:Ab','0001:ab'])]};audit['students'][0]['DTZT']={'\udfff/~':{'score':None,'unknown':'not_performed'},'0':{'score':0}};lesson={'schemaVersion':'academy.lesson-overlay/1','lessonDate':'2041-02-03','preparedAt':'2041-02-03T04:05:06.000Z','students':[]}
# ASCII JSON writes preserve lone UTF-16 units without Python replacement/normalization.
root=d.fixture('unseen-saved',json.dumps(audit,separators=(',',':')).encode()+b'\n',json.dumps(lesson,separators=(',',':')).encode()+b'\n');queries=[{'query':{'kind':'student_records','studentId':'\ud800','facet':'daily_tests'}},{'query':{'studentId':'\ud800','kind':'student_summary'}},{'query':{'kind':'student_summary','studentId':'0001 '}},{'query':{'kind':'student_history','studentId':'0001:Ab','facets':['daily_tests','books']}}]
input={'dom':[html1,html2],'savedRoot':str(root),'queries':queries};f=d.scratch/'stimuli.json';f.write_text(json.dumps(input,ensure_ascii=True,separators=(',',':')))
original=pathlib.Path(a.output);status=run(['/usr/local/bin/bun','--no-env-file',str(ROOT/'migration/source-adapter-rust-v1/original-unseen.ts'),a.source_root,str(REF),str(f),'@SCRATCH@/effects.ndjson'],original,90);assert status['exitCode']==0 and not(original/'stderr.bin').read_bytes();expected=json.loads((original/'stdout.bin').read_bytes());assert expected['effects']==ZERO
actual={'dom':[d.call({'op':'dom','html':h})['value']for h in input['dom']],'queries':[d.saved(root,q)for q in queries],'effects':ZERO}
for x,y in zip(actual['queries'],expected['queries']):
 if x['outcome']=='returned'and y['outcome']=='returned':
  assert all(isinstance(z['value']['retrievedAt'],str)and re.fullmatch(r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z',z['value']['retrievedAt'])and datetime.datetime.fromisoformat(z['value']['retrievedAt'].replace('Z','+00:00'))for z in [x,y]);x['value']['retrievedAt']=y['value']['retrievedAt']='<SOURCE_CLOCK>'
differences=diff(actual,expected);(d.scratch/'DIFF.json').write_text(json.dumps(differences,ensure_ascii=True,indent=2)+'\n');(d.scratch/'RUST-CHILDREN.json').write_text(json.dumps(d.calls,indent=2)+'\n');print(json.dumps({'domCases':2,'queryCases':4,'differences':differences[:8],'pass':not differences}));sys.exit(0 if not differences else 1)
