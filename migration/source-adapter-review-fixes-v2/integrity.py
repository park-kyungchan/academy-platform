"""Read back exact protected originals/dependencies/Cargo and immutable old/new oracles."""
import sys
sys.dont_write_bytecode=True
import pathlib,json,hashlib,subprocess,fnmatch
ROOT=pathlib.Path(__file__).resolve().parents[2];REF=ROOT/'migration/source-adapter-reference-v1';ORIGINAL=pathlib.Path(sys.argv[1]);NOTES=pathlib.Path(sys.argv[2]);sha=lambda p:hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
pins=json.loads((REF/'SOURCE-PINS.json').read_text());sources={}
for path,v in {**pins['sources'],**pins['readOnlyLeads']}.items():
 assert sha(ORIGINAL/path)==v['sha256'],path
 assert sha(ROOT/path)==v['sha256'],path
 sources[path]=v['sha256']
dependencies={}
for v in pins['dependencies']:
 for path,h in [('packagePath','packageSha256'),('requireEntry','requireEntrySha256'),('bunResolvedEntry','bunResolvedEntrySha256')]:assert sha(v[path])==v[h],v[path];dependencies[v[path]]=v[h]
for v in json.loads((REF/'DEPENDENCY-PINS-v2.json').read_text())['packages']:
 for path,h in [('observedLoadedFile','observedLoadedFileSha256'),('packageJson','packageSha256'),('resolvedEntry','entrySha256')]:assert sha(v[path])==v[h],v[path];dependencies[v[path]]=v[h]
for name,h in json.loads((REF/'FREEZE.json').read_text())['hashes'].items():assert sha(REF/name)==h,name
for name,v in json.loads((REF/'ARTIFACT-PINS.json').read_text())['files'].items():assert sha(REF/name)==v['sha256'],name
assert sha(REF/'EXPECTED.json')=='31cfecc3357007332d81dd7437eb14a1bde27b51a3ae518695c762976673f535'
assert sha(REF/'OBSERVERS-v2-EXPECTED.json')=='e322211a87cc92690de35d4fd29ff5c721dae5c7665ac95ed5e2676d7ebdc4d9'
allocation=json.loads((NOTES/'SOURCE-ALLOCATION-v4.json').read_text());cargo=json.loads((NOTES/'REGEXP-LOCK-READBACK.json').read_text())['cargoPins']
for path,h in cargo.items():assert sha(ROOT/path)==h,path
root_lint=allocation['rootLintTransition']['paths']
for path,h in root_lint.items():assert sha(ROOT/path)==h,path
changes=subprocess.run(['git','diff','--name-only',allocation['base']],cwd=ROOT,capture_output=True,text=True,check=True).stdout.splitlines()
assert all(any(fnmatch.fnmatch(path,pattern)for pattern in allocation['allowedSourceWrites'])for path in changes),changes
new={}
for freeze in ['FREEZE.json','REGEXP-FREEZE.json','AST-FREEZE.json','UNICODE-FREEZE.json','BUDGET-FREEZE.json','CONTAINERS-FREEZE.json']:
 for path,h in json.loads((ROOT/'migration/source-adapter-review-fixes-v2'/freeze).read_text())['files'].items():assert sha(ROOT/'migration/source-adapter-review-fixes-v2'/path)==h,path;new[path]=h
print(json.dumps({'pass':True,'base':allocation['base'],'sourcePins':sources,'dependencyPins':dependencies,'cargoPins':cargo,'rootLintPins':root_lint,'newFrozenPins':new,'oldOracles':{n:sha(REF/n)for n in ['EXPECTED.json','EXPECTED-EFFECTS.ndjson','OBSERVERS-v2-EXPECTED.json','OBSERVERS-v2-EFFECTS.ndjson','NORMALIZATION.json','judge.py']},'changedTrackedPaths':changes},sort_keys=True))
