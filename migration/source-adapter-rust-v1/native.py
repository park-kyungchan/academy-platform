"""Migration-only native execution and immutable judge access; product has no reference import."""
import sys
sys.dont_write_bytecode = True
import importlib.util, pathlib, json, hashlib
ROOT = pathlib.Path(__file__).resolve().parents[2]
def module(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value
executor = module(ROOT / 'migration/source-adapter-reference-v2/run.py', 's09_qualified_v2')
def run(command, output, timeout=90, cwd=None, output_limit=16*1024*1024):
    def snapshot():
        files=[]
        for pattern in ['crates/edu-academy-sources/src/**/*.rs','crates/edu-academy-sources/tests/**/*.rs','migration/source-adapter-rust-v1/*','docs/s09/*']:
            files.extend(p for p in ROOT.glob(pattern) if p.is_file())
        return {str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(set(files))}
    before=snapshot()
    status=executor.execute(command, output, timeout, output_limit, cwd)
    after=snapshot()
    protected=['Cargo.toml','Cargo.lock','crates/edu-academy-sources/Cargo.toml','crates/edu-academy-core/Cargo.toml']
    provenance={'sourceBefore':before,'sourceAfter':after,'sourceUnchangedDuringRun':before==after,'protectedFiles':{p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in protected}}
    (pathlib.Path(output)/'PROVENANCE.json').write_text(json.dumps(provenance,indent=2)+'\n')
    return status
if __name__ == '__main__':
    import argparse
    p = argparse.ArgumentParser()
    p.add_argument('--output', required=True)
    p.add_argument('--timeout', type=float, default=90)
    p.add_argument('command', nargs=argparse.REMAINDER)
    a = p.parse_args()
    command = a.command[1:] if a.command[:1] == ['--'] else a.command
    print(json.dumps(run(command, a.output, a.timeout, ROOT)))
