#!/usr/bin/env python3
"""Static bounded import closure and existing installed package byte identities."""
import argparse,hashlib,json,pathlib,re
sha=lambda b:hashlib.sha256(b).hexdigest()
def record(p):
 b=p.read_bytes();return {'sha256':sha(b),'bytes':len(b)}
def collect(root,deps):
 mcp=root/'modules/academy-workbench/tools/academy-mcp'
 queue=[mcp/'src'/n for n in ['neutral-server.ts','dailyrecord-server.ts','runtime-release.ts','stable-launcher.ts']]
 queue += [mcp/'tests'/n for n in ['neutral-upstream-fixture.ts','neutral-paper-header-fixture.ts','neutral-saved-fixture.ts','network-deny.ts']]
 seen={};bare=set()
 while queue:
  p=queue.pop().resolve()
  if str(p.relative_to(root)) in seen:continue
  seen[str(p.relative_to(root))]=record(p)
  for name in re.findall(r'^import\s+(?:[^\n]*?\sfrom\s*)?[\'"]([^\'"]+)[\'"]',p.read_text(),re.M):
   if name.startswith('.'):
    candidate=p.parent/name
    for q in [candidate,pathlib.Path(str(candidate)+'.ts'),candidate/'index.ts']:
     if q.is_file():queue.append(q);break
   elif not name.startswith(('node:','bun')):bare.add(name.split('/')[0] if not name.startswith('@') else '/'.join(name.split('/')[:2]))
 for p in (mcp/'runtime').iterdir():
  if p.is_file():seen[str(p.relative_to(root))]=record(p)
 for p in [root/'bun.lock',mcp/'package.json',root/'Cargo.toml',root/'Cargo.lock']:
  if p.is_file():seen[str(p.relative_to(root))]=record(p)
 packages={};missing=[]
 def find(name,start):
  for d in [start,*start.parents]:
   p=d/'node_modules'/name/'package.json'
   if p.is_file():return p
  return None
 todo=[(n,deps) for n in sorted(bare)]
 while todo:
  name,start=todo.pop();manifest=find(name,start)
  if manifest is None:missing.append({'name':name,'from':str(start)});continue
  path=manifest.parent.resolve()
  if str(path) in packages:continue
  d=json.loads(manifest.read_text());files={}
  for p in sorted(path.rglob('*')):
   if p.is_file() and 'node_modules' not in p.relative_to(path).parts:files[str(p.relative_to(path))]=record(p)
  packages[str(path)]={'name':d['name'],'version':d['version'],'files':files}
  todo.extend((n,path) for n in d.get('dependencies',{}))
 return {'schema':'s11.source-and-dependency-pins/1','sourceRoot':str(root),'dependencyRoot':str(deps),'sources':seen,'packages':packages,'unresolvedDeclaredDependencies':missing,'method':'static relative import closure (including types); complete installed package files for reached bare dependencies; dynamic native imports separately held in GRAPH.json'}
if __name__=='__main__':
 a=argparse.ArgumentParser();a.add_argument('--source-root',required=True,type=pathlib.Path);a.add_argument('--output',required=True,type=pathlib.Path);a.add_argument('--dependency-root',default='/home/palantirkc/projects/education-platform/modules/academy-workbench/tools/academy-mcp',type=pathlib.Path);x=a.parse_args()
 d=collect(x.source_root.resolve(),x.dependency_root.resolve());x.output.write_text(json.dumps(d,indent=2)+'\n');print(json.dumps({'sources':len(d['sources']),'packages':len(d['packages']),'missing':d['unresolvedDeclaredDependencies']}))
