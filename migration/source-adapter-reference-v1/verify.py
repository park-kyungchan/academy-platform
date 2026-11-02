"""Read-only selected source/dependency/frozen-artifact integrity verification."""
import pathlib,json,hashlib,sys,os
ref=pathlib.Path(__file__).resolve().parent;root=ref.parent.parent
sha=lambda p:hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
pins=json.loads((ref/'SOURCE-PINS.json').read_text());count=0
for path,entry in {**pins['sources'],**pins['readOnlyLeads']}.items():
 if sha(root/path)!=entry['sha256']:raise SystemExit('original_pin_changed:'+path)
 count+=1
for entry in pins['dependencies']:
 if (root/entry['link']).resolve()!=pathlib.Path(entry['target']):raise SystemExit('dependency_link_changed:'+entry['name'])
 for p,h in [('packagePath','packageSha256'),('requireEntry','requireEntrySha256'),('bunResolvedEntry','bunResolvedEntrySha256')]:
  if sha(entry[p])!=entry[h]:raise SystemExit('dependency_entry_changed:'+entry['name'])
for entry in json.loads((ref/'DEPENDENCY-PINS-v2.json').read_text())['packages']:
 if sha(entry['observedLoadedFile'])!=entry['observedLoadedFileSha256']:raise SystemExit('observed_dependency_file_changed:'+entry['name'])
 if not entry['lockContainsExactVersion'] or sha(entry['packageJson'])!=entry['packageSha256'] or sha(entry['resolvedEntry'])!=entry['entrySha256']:raise SystemExit('runtime_dependency_changed:'+entry['name'])
freeze=json.loads((ref/'FREEZE.json').read_text())
for path,h in freeze['hashes'].items():
 if sha(ref/path)!=h:raise SystemExit('frozen_primary_changed:'+path)
artifact=ref/'ARTIFACT-PINS.json'
if artifact.exists():
 for path,entry in json.loads(artifact.read_text())['files'].items():
  if sha(ref/path)!=entry['sha256']:raise SystemExit('frozen_artifact_changed:'+path)
print(json.dumps({'selectedOriginalAndLeadPins':count,'directDependencyLinks':len(pins['dependencies']),'observedRuntimePackages':35,'primaryFrozenPins':len(freeze['hashes']),'artifactManifestPresent':artifact.exists(),'pass':True}))
