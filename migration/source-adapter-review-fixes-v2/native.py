"""Bounded v2 native execution plus this correction's exact source/tool/oracle pins."""
import sys
sys.dont_write_bytecode=True
import pathlib,json,hashlib,argparse,importlib.util
ROOT=pathlib.Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('immutable_s09_executor',ROOT/'migration/source-adapter-reference-v2/run.py');executor=importlib.util.module_from_spec(spec);spec.loader.exec_module(executor)
def pins():
 paths=[]
 for glob in ['crates/edu-academy-sources/src/**/*.rs','crates/edu-academy-sources/tests/*.rs','crates/edu-academy-core/src/**/*.rs','migration/source-adapter-review-fixes-v2/*','migration/source-adapter-reference-v1/*','migration/source-adapter-reference-v2/*','migration/source-adapter-rust-v1/*']:
  paths.extend(p for p in ROOT.glob(glob) if p.is_file())
 paths += [ROOT/p for p in ['Cargo.toml','Cargo.lock','crates/edu-academy-sources/Cargo.toml','crates/edu-academy-core/Cargo.toml']]
 return {str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest()for p in sorted(set(paths))}
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--output',required=True);p.add_argument('--timeout',type=float,default=90);p.add_argument('command',nargs=argparse.REMAINDER);a=p.parse_args();before=pins();command=a.command[1:]if a.command[:1]==['--']else a.command
 result=executor.execute(command,a.output,a.timeout,cwd=ROOT);after=pins();pathlib.Path(a.output,'PROVENANCE.json').write_text(json.dumps({'sourceBefore':before,'sourceAfter':after,'sourceUnchangedDuringRun':before==after},indent=2)+'\n');print(json.dumps(result))
