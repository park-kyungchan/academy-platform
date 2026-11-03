#!/usr/bin/env python3
"""Read public selection metadata/source only. Never open referenced admissions,
owner configs, payloads, locks, journals, credential stores or container files."""
import ast,hashlib,json,pathlib,re,sys
HERE=pathlib.Path(__file__).resolve().parent
digest=lambda b:hashlib.sha256(b).hexdigest()
def pin(p):
 b=p.read_bytes();s=p.stat();return {'path':str(p),'bytes':len(b),'sha256':digest(b),'uid':s.st_uid,'mode':oct(s.st_mode&0o777),'resolved':str(p.resolve())}
def public_js(p):
 if not p.is_file():return {'path':str(p),'observed':False}
 b=p.read_bytes();s=b.decode()
 imports=sorted(set(re.findall(r'^import\s+(?:[^\n]*?\sfrom\s*)?[\'"]([^\'"]+)[\'"]',s,re.M)))
 requires=sorted(set(re.findall(r'\b(?:__)?require\([\'"]([^\'"]+)[\'"]\)',s)))
 return {**pin(p),'staticImportSpecifiers':imports,'literalRequireCandidates':requires,'dynamicImportQualification':'native config data-URL imports and external jsdom require separate owner proof','qualification':'public_source_bytes_only_not_executed'}
base=pathlib.Path('/root/.local/share/academy-mcp-scoped-20261030')
pointer=json.loads((base/'release.json').read_text());release=base/'releases'/pointer['generation']
ep_path=pathlib.Path(pointer['endpoints']['path']);endpoints=json.loads(ep_path.read_text())
registry_path=pathlib.Path(pointer['registry']['path']);registry=json.loads(registry_path.read_text())
project=pathlib.Path('/home/palantirkc/work/ws-replan')
metadata={}
for relative in ['preparation/slices/S10/reference-v1/LEAD-HOST-LOCK-LOCATOR.json','preparation/final-review/runtime/native-config-metadata.json']:
 p=project/relative;metadata[relative]=pin(p)
output={'schema':'s11.public-runtime-graph/1','observedPointer':pointer,'pointerFile':pin(base/'release.json'),'endpointsFile':pin(ep_path),'registryFile':pin(registry_path),
 'installedRoutingDescriptor':{'path':pointer['routing']['path'],'declaredSha256':pointer['routing']['sha256'],'opened':False,'reason':'not_needed; preserve private admission custody'},
 'selectedOriginalRoutes':[{'name':x['tool']['name'],'endpoint':x['endpoint'],'upstreamName':x['upstreamName']} for x in registry],
 'hostEntrypoints':[public_js(base/'launcher.mjs'),public_js(release/'server.mjs')],
 'metadataEvidence':metadata,'endpoints':{},'chosenCandidateContract':'fail_closed_mandatory_graph_mismatch',
 'nativeObservation':False,'containerInvoked':False,'actualAccountLockOpened':False}
for name,e in endpoints.items():
 args=e['args'];v={'command':e['command'],'environmentKeys':sorted(e.get('env',{})),'argumentSha256':[digest(a.encode()) for a in args]}
 if '-c' in args:
  code=args[args.index('-c')+1];tree=ast.parse(code);v['inlinePythonSha256']=digest(code.encode());v['imports']=[];v['nativeDeclaredPins']={};native=None
  for n in tree.body:
   if isinstance(n,(ast.Import,ast.ImportFrom)):v['imports'].append(ast.unparse(n))
   if isinstance(n,ast.Assign):
    for target in n.targets:
     if isinstance(target,ast.Name) and target.id=='native':native=ast.literal_eval(n.value)
     if isinstance(target,ast.Name) and target.id=='pins':v['nativeDeclaredPins']=json.loads(ast.literal_eval(n.value.args[0]))
  v['nativePublicRoot']=native
  for n in ast.walk(tree):
   if isinstance(n,ast.Call) and ast.unparse(n.func)=='os.execvpe':
    # argv contains public flag/path/hash locators only, env is deliberately omitted.
    v['nativeExecExpression']=ast.unparse(n.args[1])
   if isinstance(n,ast.Call) and ast.unparse(n.func)=='get_env_value_prefer_dotenv':
    v['credentialLookup']={'publicFunction':'hermes_cli.config.get_env_value_prefer_dotenv','keyNames':[ast.literal_eval(a) for a in n.args if isinstance(a,ast.Constant)],'valuesRead':False}
  v['hostTransport']={'accountLockLocator':args[2],'accountLockOpened':False,'flockWaitSeconds':45,'container':args[14],'uid':args[7],'cwd':args[13],'pythonExecutable':args[15],
   'environmentKeys':[args[i+1].split('=',1)[0] for i,a in enumerate(args[:-1]) if a=='--env']}
  v['declaredPinQualification']='metadata_only_container_bytes_owners_binaries_unverified'
 else:
  v['argv']=args;v['publicExecutable']=public_js(pathlib.Path(args[-1]))
 output['endpoints'][name]=v
output['sourceOwnedRuntimeEdges']=[
 {'edge':'Hermes stdio relay -> Unix socket -> socket-activated host launcher','source':'modules/academy-workbench/tools/academy-mcp/runtime','hostSocket':'/docker/hermes-agent-2q3y/data/academy-mcp-relay/bridge.sock','containerSocket':'/opt/data/academy-mcp-relay/bridge.sock','mountQualification':'documented existing /opt/data bind edge; actual mount/namespace not inspected','hostUser':'root','containerDirectoryOwnerUid':10000,'socketDirectoryMode':'0700','sourceSocketMode':'0666','actualUnitSelection':'unverified'},
 {'edge':'dailyrecord-server -> nativeDailyRecordConfiguration -> pinned AcademyHttpClient bundle -> external jsdom','source':'modules/academy-workbench/tools/academy-mcp/src/native-dailyrecord-backend.ts:553','requiredUid':10000,'requiredHermesHome':'/opt/data/profiles/dc-gang-a-dev','jsdomPath':'/opt/data/workspace/dc-gang-a/node_modules/jsdom/lib/api.js','requiredJsdomVersion':'30.1.2','externalImportCountContract':1,'credentialEnvKey':'DC_ACADEMY_HTTP_PASSWORD','nativeConfigOwner':'existing Hermes profile','contentRead':False},
 {'edge':'dailyrecord-server -> runtimeIdentityConfiguration -> learning config/policy/admissions -> owner workflow/CISM/textbook stores','source':'modules/academy-workbench/tools/academy-mcp/src/dailyrecord-server.ts','ownerArtifactsRoot':'/opt/data/profiles/dc-gang-a-dev/artifacts/','ownerState':'retained existing workflow, account lock and native journal formats; no new generic lifecycle'},
]
output['remainingNativePrerequisites']=[
 'Owner-authorized metadata verification of actual container binary/UID/cwd/mount and Hermes selected config/relay/unit; no inferred host/container path equivalence.',
 'Verify public hermes_cli.config source at the selected container revision and its native dotenv/session/permission ownership without opening secrets under this grant.',
 'Verify actual declared bundle bytes and external imports/dependency versions in owner namespace; descriptor hashes alone are not current container observations.',
 'Independently prove account-qualified authentication, authorization, exact source content, native UID and genuine transport seals before activating any S09/S10 source path.',
 'Retain original account lock/journal readers, keys, unresolved field fences and immutable old sessions; exact 65081389 journal gate remains unchanged.',
 'Root independent Wonder review and implementation authorization; separate P9 read/write/recovery/stability/deletion conditions.'
]
p=pathlib.Path(sys.argv[1]);p.write_text(json.dumps(output,indent=2,ensure_ascii=False)+'\n')
print(json.dumps({'routes':len(registry),'endpointKeys':list(endpoints),'nativePinDeclarations':sum(len(v.get('nativeDeclaredPins',{})) for v in output['endpoints'].values()),'nativeExecuted':False}))
