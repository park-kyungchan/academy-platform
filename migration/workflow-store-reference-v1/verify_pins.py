#!/usr/bin/env python3
"""Read only the selected source/dependency/reference pins, never the repository at large."""
import hashlib,json,pathlib
HERE=pathlib.Path(__file__).resolve().parent;REPO=HERE.parent.parent
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 pins=json.loads((HERE/'SOURCE-PINS.json').read_text());count=0
 for p in pins['sources']:
  if sha(REPO/p['path'])!=p['sha256']:raise RuntimeError('s10_selected_source_changed:'+p['path'])
  count+=1
 dep=pins['dependency'];link=REPO/dep['ownedIgnoredLink']
 if not link.is_symlink() or str(link.resolve())!=dep['linkTarget']:raise RuntimeError('s10_dependency_link_changed')
 if json.loads((link/'package.json').read_text())['version']!=dep['version']:raise RuntimeError('s10_dependency_version_changed')
 for e in dep['entries']:
  if sha(pathlib.Path(e['path']))!=e['sha256']:raise RuntimeError('s10_selected_dependency_changed')
 for old in json.loads((HERE/'OLD-STATES.json').read_text()):
  for f in old['files']:
   if sha(HERE/'fixtures'/old['name']/f['path'])!=f['sha256']:raise RuntimeError('s10_old_fixture_changed')
 artifacts=json.loads((HERE/'ARTIFACT-PINS.json').read_text())
 for e in artifacts:
  if sha(HERE/e['path'])!=e['sha256']:raise RuntimeError('s10_reference_artifact_changed:'+e['path'])
 print(json.dumps({'selectedSources':count,'dependencyEntries':len(dep['entries']),'referenceArtifacts':len(artifacts),'verified':True}))
if __name__=='__main__':main()
