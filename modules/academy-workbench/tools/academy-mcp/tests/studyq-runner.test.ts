import {afterAll,beforeAll,describe,expect,test} from 'bun:test';
import {chmod,mkdir,mkdtemp,readFile,realpath,rm,symlink,writeFile} from 'node:fs/promises';
import {existsSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {pinnedStudyqRunner,studyqRunnerConfiguration,studyqSourceTreeSha256,verifyStudyqSourceTree} from '../src/studyq-runner.ts';

// src/studyq-runner.ts: the hash-pinned interpreter and studyq_linux source tree, run over a
// synthetic studyq_linux package (no PyMuPDF), so `bun test tests` stays deterministic. The runner
// runs `assemble-print` only: the official reports are made on the PC (src/pc-executor.ts).
const sha=(bytes:Buffer|string)=>createHash('sha256').update(bytes).digest('hex');
const cli=`import hashlib, json, os, sys
def main(argv):
    data = sys.stdin.buffer.read()
    out = argv[argv.index('--out') + 1]
    with open(out + '.env.json', 'w') as handle:
        json.dump({'env': sorted(os.environ), 'isolated': sys.flags.isolated, 'noBytecode': sys.dont_write_bytecode, 'argv': argv}, handle)
    if os.path.lexists(out):
        print(json.dumps({'status': 'error', 'code': 'output_exists', 'detail': 'output exists'}))
        return 1
    pdf = b'%PDF-1.4 synthetic ' + hashlib.sha256(data).hexdigest().encode()
    with open(out, 'wb') as handle:
        handle.write(pdf)
    print(json.dumps({'status': 'ok', 'reportType': 'print_exam', 'pages': 2, 'sha256': hashlib.sha256(pdf).hexdigest(), 'outputPath': out}))
    return 0
`;
let root:string,interpreter:string,interpreterSha256:string;
async function tree(name:string,files:Record<string,string>={}){
  const src=resolve(root,name),pkg=resolve(src,'studyq_linux');
  await mkdir(pkg,{recursive:true});await chmod(src,0o755);await chmod(pkg,0o755);
  for(const [file,text] of Object.entries({'__init__.py':'','cli.py':cli,'errors.py':'',...files})){
    await writeFile(resolve(pkg,file),text);await chmod(resolve(pkg,file),0o644);
  }
  return {src,sourceSha256:await studyqSourceTreeSha256(src)};
}
beforeAll(async()=>{
  root=await realpath(await mkdtemp(resolve(tmpdir(),'studyq-runner-')));
  interpreter=await realpath('/usr/bin/python3');
  interpreterSha256=sha(await readFile(interpreter));
});
afterAll(async()=>{await rm(root,{recursive:true,force:true});});
const assets=()=>resolve(root,'assets');

describe('the hash-pinned studyq runner',()=>{
  test('studyq_runner_runs_only_assemble_print_isolated_with_a_clean_environment',async()=>{
    const {src,sourceSha256}=await tree('ok');
    const runner=pinnedStudyqRunner({interpreter,interpreterSha256,srcDir:src,sourceSha256,assetsDir:assets()});
    // The report render methods are gone with the Linux report renderers.
    expect(Object.keys(runner)).toEqual(['assemblePrint']);
    const out=resolve(root,'ok.pdf');
    expect(await runner.assemblePrint({kind:'exam'},out)).toEqual({status:'rendered',reportType:'print_exam',pages:2,sha256:sha(await readFile(out)),outputPath:out});
    const seen=JSON.parse(await readFile(out+'.env.json','utf8'));
    expect(seen).toMatchObject({isolated:1,noBytecode:true,argv:['assemble-print','--out',out]});
    expect(seen.env.filter((name:string)=>name!=='LC_CTYPE')).toEqual(['LANG','PATH']);
    expect(existsSync(resolve(src,'studyq_linux','__pycache__'))).toBe(false);
    expect(await runner.assemblePrint({kind:'exam'},out)).toMatchObject({status:'refused',code:'output_exists'});
    await expect(runner.assemblePrint({},'relative.pdf')).rejects.toThrow('output_path_invalid');
  });

  test('studyq_runner_refuses_a_changed_or_unsafe_tree_before_any_spawn',async()=>{
    const cases:[string,(src:string)=>Promise<void>][]=[
      ['a changed file',async src=>writeFile(resolve(src,'studyq_linux','cli.py'),cli+'# changed\n')],
      ['an added module',async src=>{await writeFile(resolve(src,'studyq_linux','extra.py'),'');await chmod(resolve(src,'studyq_linux','extra.py'),0o644);}],
      ['a symbolic link',async src=>symlink('/etc/hostname',resolve(src,'studyq_linux','link.py'))],
      ['a __pycache__ directory',async src=>mkdir(resolve(src,'studyq_linux','__pycache__'),{mode:0o755})],
      ['a .pyc file',async src=>{await writeFile(resolve(src,'studyq_linux','cli.cpython-312.pyc'),'');await chmod(resolve(src,'studyq_linux','cli.cpython-312.pyc'),0o644);}],
      ['a group-writable file',async src=>chmod(resolve(src,'studyq_linux','errors.py'),0o664)],
      ['a world-writable directory',async src=>chmod(resolve(src,'studyq_linux'),0o757)],
    ];
    for(const [index,[name,change]] of cases.entries()){
      const {src,sourceSha256}=await tree('unsafe-'+index);
      const runner=pinnedStudyqRunner({interpreter,interpreterSha256,srcDir:src,sourceSha256,assetsDir:assets()});
      await change(src);
      const out=resolve(root,'unsafe-'+index+'.pdf');
      await expect(runner.assemblePrint({kind:'exam'},out)).rejects.toThrow('studyq_source_rejected');
      expect({name,spawned:existsSync(out+'.env.json')}).toEqual({name,spawned:false});
    }
    const {src,sourceSha256}=await tree('interpreter');
    const wrong=pinnedStudyqRunner({interpreter,interpreterSha256:'0'.repeat(64),srcDir:src,sourceSha256,assetsDir:assets()});
    await expect(wrong.assemblePrint({kind:'exam'},resolve(root,'interpreter.pdf'))).rejects.toThrow('studyq_interpreter_rejected');
    expect(existsSync(resolve(root,'interpreter.pdf.env.json'))).toBe(false);
    await expect(verifyStudyqSourceTree(src,'a'.repeat(64))).rejects.toThrow('studyq_source_rejected');
  });

  test('studyq_runner_launch_option',async()=>{
    const {src,sourceSha256}=await tree('launch');
    const {remainingArgs,runner}=await studyqRunnerConfiguration(['--other','--studyq-runner',interpreter,interpreterSha256,src,sourceSha256,assets(),'--last'],{});
    expect(remainingArgs).toEqual(['--other','--last']);expect(runner).not.toBeNull();
    expect((await studyqRunnerConfiguration(['--other'])).runner).toBeNull();
    await expect(studyqRunnerConfiguration(['--studyq-runner',interpreter,interpreterSha256,src,sourceSha256])).rejects.toThrow('studyq_runner_arguments_invalid');
    await expect(studyqRunnerConfiguration(['--studyq-runner',interpreter,interpreterSha256,'relative',sourceSha256,assets()])).rejects.toThrow('studyq_runner_arguments_invalid');
    await expect(studyqRunnerConfiguration(['--studyq-runner',interpreter,interpreterSha256,src,'b'.repeat(64),assets()])).rejects.toThrow('studyq_source_rejected');
  });

});
