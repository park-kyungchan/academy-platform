"""Deliberately incorrect migration-only child. Product output is changed after execution."""
import sys
sys.dont_write_bytecode=True
import pathlib,json,subprocess,os,time,hashlib
mode,binary,directory=sys.argv[1:];root=pathlib.Path(directory);raw=sys.stdin.buffer.read();start=time.monotonic();p=subprocess.Popen([binary],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE);pgid=os.getpgid(p.pid);out,err=p.communicate(raw,timeout=50);record=root/str(p.pid);record.mkdir(mode=0o700)
sha=lambda b:hashlib.sha256(b).hexdigest()
for n,b in [('stdin.bin',raw),('stdout.bin',out),('stderr.bin',err)]: (record/n).write_bytes(b)
(record/'RESULT.json').write_text(json.dumps({'argv':[binary],'pid':p.pid,'pgid':pgid,'exitCode':p.returncode,'elapsedSeconds':time.monotonic()-start,'binarySha256':sha(pathlib.Path(binary).read_bytes()),'stdinSha256':sha(raw),'stdoutSha256':sha(out),'stderrSha256':sha(err)},indent=2)+'\n')
if p.returncode:sys.exit(p.returncode)
v=json.loads(out);request=json.loads(raw)
if request['op']=='presentation' and v['outcome']=='returned':
 value=v['value']['value']
 if mode=='value':value['difficultyMapping']='invented_semantics'
 if mode=='span' and value['functionEvidence']:value['functionEvidence'][0]['sourceRange']['start']+=1
 if mode=='missing':value.pop('functionEvidence')
 if mode=='refusal':value['unknowns']=[x for x in value['unknowns']if not x.endswith('parse_unqualified')]
if request['op']=='http_session' and mode=='effect':
 with open(request['effectTrace'],'a')as f:f.write('{"event":"mutating_fake_dispatch"}\n')
sys.stdout.write(json.dumps(v,ensure_ascii=True,separators=(',',':'))+'\n')
