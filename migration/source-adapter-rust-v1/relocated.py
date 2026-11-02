"""Relocate the Rust executable and launch from a third cwd; evidence root stays explicit."""
import sys
sys.dont_write_bytecode=True
import pathlib,argparse,shutil,os,json
from compare import Driver,REF,js,ZERO
p=argparse.ArgumentParser();p.add_argument('--binary',required=True);p.add_argument('--scratch',required=True);p.add_argument('--metadata',required=True);a=p.parse_args();scratch=pathlib.Path(a.scratch);exe=scratch/'executable-moved';exe.mkdir(mode=0o700);binary=exe/'source-reader';shutil.copy2(a.binary,binary);evidence=scratch/'evidence-moved';evidence.mkdir(mode=0o700);cwd=scratch/'cwd-third';cwd.mkdir(mode=0o700);os.chdir(cwd);d=Driver(binary,evidence);frames=d.frames(json.loads((REF/'corpus/CASES.json').read_text()));metadata=json.loads(pathlib.Path(a.metadata).read_text());assert metadata['owner']=='original-javascript-prototype-metadata-only'and metadata['effects']==ZERO
# Judge's trace locator is fixed, while actual evidence was deliberately relocated.
shutil.copyfile(d.effect,scratch/'effects.ndjson');(scratch/'RUST-CHILDREN.json').write_text(json.dumps(d.calls,indent=2)+'\n');print(js({'schema':'s09.original-reference-output/1','frames':frames,'runtimeExports':metadata['runtimeExports']}))
