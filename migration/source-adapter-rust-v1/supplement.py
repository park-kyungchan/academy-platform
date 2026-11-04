"""Independent disposable roots/content binding and observed no-effect/lifetime assertions."""
import sys
sys.dont_write_bytecode=True
import argparse,pathlib,json,os,shutil
from compare import Driver,REF,ROOT,TARGET,returned,sha,js
p=argparse.ArgumentParser();p.add_argument('--binary',required=True);p.add_argument('--scratch',required=True);a=p.parse_args();d=Driver(a.binary,a.scratch);checks=[]
def check(name,condition):
 checks.append({'name':name,'pass':bool(condition)})
 if not condition:raise AssertionError(name)
root=d.scratch/'owner';root.mkdir(mode=0o700)
files=[]
for source,name in [('current.html','main.html'),('current.xml','selected.xml')]:
 b=(REF/'corpus'/source).read_bytes();(root/name).write_bytes(b);(root/name).chmod(0o600);files.append({'path':name,'sha256':sha(b)})
request={'op':'unadmitted_day_record','root':str(root),'account':'fictional-account-A','instance':'fictional-instance','existingOwnerAssociation':{'state':'fictional_data_only_not_trusted_owner_proof'},'files':files,'selection':TARGET}
x=d.call(request);check('unadmitted_real_dom_projection',x['outcome']=='returned');x=x['value'];check('fields_recomputed_from_captured_bytes',x['fields']['progress']=='Unit & progress 가😀'and x['fields']['homework']=='');check('native_identity_unchanged',x['legacyIdentity']==TARGET);check('physical_xml_tuple',x['physicalTuple']['record_seq']=='13'and x['physicalTuple']['stu_pri_no']=='77001');check('predecode_raw_hash',x['capturedMainSha256']==files[0]['sha256']);check('no_authority',x['status']=='UNADMITTED'and x['authority']=='none'and x['nativeOwnerVerified']is False)
request['account']='fictional-account-B';y=d.call(request)['value'];check('account_namespace_separate_same_native_key',x['sourceAccount']!=y['sourceAccount']and x['legacyIdentity']==y['legacyIdentity']);check('raw_utf16_span_roundtrip',all(text['raw']==(root/'main.html').read_bytes().decode().encode('utf-16-le')[text['sourceRange']['start']*2:text['sourceRange']['end']*2].decode('utf-16-le')for text in x['rawControls']))
invalid=d.call({**request,'account':''});check('explicit_association_data_required',invalid['outcome']=='threw'and invalid['error']['message']=='evidence_explicit_account_association_required')
(root/'main.html').write_bytes((root/'main.html').read_bytes().replace(b'Unit &amp; progress',b'Tampered progress'));check('content_hash_tamper_refused',d.call(request)['outcome']=='threw')
legacy=d.fixture('saved-owner')
for path in sorted(legacy.rglob('*')):
 path.chmod(0o700 if path.is_dir()else 0o600)
manifest=legacy/'tools/academy-mcp/architecture/source-pins.json';q={'op':'owner_saved','root':str(legacy),'manifestSha256':sha(manifest.read_bytes()),'input':{'query':{'kind':'roster'}}};strict=d.call(q);normal=d.saved(legacy,q['input']);check('strict_and_legacy_same_snapshot_and_roster',strict['outcome']=='returned'and strict['value']['records']==normal['value']['records']and strict['value']['snapshot']==normal['value']['snapshot']);manifest.write_bytes(manifest.read_bytes()+b' ');check('strict_manifest_hash_refused',d.call(q)['outcome']=='threw')
# Exec and evidence roots relocate independently; cwd is a third directory.
exe=d.scratch/'executable-moved';exe.mkdir(mode=0o700);moved=exe/'reader';shutil.copy2(a.binary,moved);cwd=d.scratch/'cwd-third';cwd.mkdir(mode=0o700);old=os.getcwd();os.chdir(cwd);d.binary=str(moved);relocated=d.saved(legacy,q['input']);os.chdir(old);check('relocated_binary_root_cwd_saved_semantics',relocated['value']['records']==normal['value']['records']and relocated['value']['snapshot']==normal['value']['snapshot']);check('real_supported_inventory',d.call({'op':'inventory'})['value']==['inventory','native_config_schema','native_configuration_probe','legacy_paths','saved_query','saved_records','paginate','tool_bytes','literal_key','resolve_name','pinned_local','owner_saved','owner_capture','unadmitted_day_record','dom','source_function','presentation','http_session','native_session','operation_admission'])
(d.scratch/'RUST-CHILDREN.json').write_text(json.dumps(d.calls,indent=2)+'\n');print(js({'checks':checks,'nativeEffects':False,'authority':'none'}))
