import {afterAll,beforeAll,describe,expect,test} from 'bun:test';
import {chmod,mkdir,mkdtemp,readFile,readdir,realpath,rm,symlink,writeFile} from 'node:fs/promises';
import {existsSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {pinnedStudyqRunner,studyqRunnerConfiguration,studyqSourceTreeSha256,verifyStudyqSourceTree} from '../src/studyq-runner.ts';
import {projectCourseBookReport} from '../../../../../apps/academy/dc-gang-a/src/lms/courseBookReport.ts';

// src/studyq-runner.ts: the hash-pinned interpreter and studyq_linux source tree, run over a
// synthetic studyq_linux package (no PyMuPDF), so `bun test tests` stays deterministic. The
// render of the real executor runs only with CHECK_STUDYQ_PYMUPDF=1 and STUDYQ_ASSETS_DIR.
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
    if json.loads(data).get('student') == 'refuse':
        print(json.dumps({'status': 'error', 'code': 'input_invalid', 'detail': 'range:bunit_list[0].questions[0].type'}))
        return 1
    pdf = b'%PDF-1.4 synthetic ' + hashlib.sha256(data).hexdigest().encode()
    with open(out, 'wb') as handle:
        handle.write(pdf)
    print(json.dumps({'status': 'ok', 'reportType': 'course_book', 'pages': 8, 'sha256': hashlib.sha256(pdf).hexdigest(), 'outputPath': out}))
    return 0
`;
const parity=`import json, sys
def main(argv):
    data = json.loads(sys.stdin.buffer.read())
    open(argv[argv.index('--pdf') + 1], 'rb').read()
    with open(argv[argv.index('--pdf') + 1] + '.parity-argv.json', 'w') as handle:
        json.dump(argv, handle)
    bad = data.get('comment') == 'mismatch'
    if data.get('comment') == 'crash':
        print('not json')
        return 1
    print(json.dumps({'mismatches': 2 if bad else 0, 'rawRn': 1 if bad else 0, 'fields': 10, 'pages': 8, 'failed': [], 'status': 'mismatch' if bad else 'ok'}))
    return 1 if bad else 0
`;
let root:string,interpreter:string,interpreterSha256:string;
async function tree(name:string,files:Record<string,string>={}){
  const src=resolve(root,name),pkg=resolve(src,'studyq_linux');
  await mkdir(pkg,{recursive:true});await chmod(src,0o755);await chmod(pkg,0o755);
  for(const [file,text] of Object.entries({'__init__.py':'','cli.py':cli,'parity.py':parity,...files})){
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
  test('studyq_runner_renders_then_reads_parity_whatever_its_exit',async()=>{
    const {src,sourceSha256}=await tree('ok');
    const runner=pinnedStudyqRunner({interpreter,interpreterSha256,srcDir:src,sourceSha256,assetsDir:assets()});
    const out=resolve(root,'ok.pdf');
    const rendered=await runner.renderCourseBook({student:'Invented Student',comment:''},out);
    expect(rendered).toEqual({status:'rendered',pages:8,sha256:sha(await readFile(out)),outputPath:out,parityOk:true,parityMismatches:0,parityRawRn:0});
    // parity.main exits 1 on a mismatch and still prints its JSON: parityOk is the real result.
    const mismatch=await runner.renderCourseBook({student:'Invented Student',comment:'mismatch'},resolve(root,'mismatch.pdf'));
    expect(mismatch).toMatchObject({status:'rendered',parityOk:false,parityMismatches:2,parityRawRn:1});
    // Stdout that is no parity JSON is a runner failure.
    await expect(runner.renderCourseBook({student:'Invented Student',comment:'crash'},resolve(root,'crash.pdf'))).rejects.toThrow('studyq_runner_failed');
    // A refusal of the executor is answered as it is.
    expect(await runner.renderCourseBook({student:'refuse'},resolve(root,'refused.pdf'))).toEqual({status:'refused',code:'input_invalid',detail:'range:bunit_list[0].questions[0].type'});
    expect(await runner.renderCourseBook({student:'x'},out)).toMatchObject({status:'refused',code:'output_exists'});
    // Isolated, no bytecode, a clean environment, and the words of the Interface.
    const seen=JSON.parse(await readFile(out+'.env.json','utf8'));
    expect(seen).toMatchObject({isolated:1,noBytecode:true,argv:['render','--type','course_book','--assets',assets(),'--out',out]});
    expect(seen.env.filter((name:string)=>name!=='LC_CTYPE')).toEqual(['LANG','PATH']);
    expect(existsSync(resolve(src,'studyq_linux','__pycache__'))).toBe(false);
    // The archive key file is forwarded only when configured.
    const keyed=pinnedStudyqRunner({interpreter,interpreterSha256,srcDir:src,sourceSha256,assetsDir:assets(),printKeyFile:'/invented/key'});
    await keyed.renderCourseBook({student:'x',comment:''},resolve(root,'keyed.pdf'));
    expect(JSON.parse(await readFile(resolve(root,'keyed.pdf.env.json'),'utf8')).env).toContain('STUDYQ_PRINT_KEY_FILE');
    await expect(runner.renderCourseBook({},'relative.pdf')).rejects.toThrow('output_path_invalid');
  });

  test('studyq_runner_renders_cism_with_the_cism_type_for_render_and_parity',async()=>{
    const {src,sourceSha256}=await tree('cism');
    const runner=pinnedStudyqRunner({interpreter,interpreterSha256,srcDir:src,sourceSha256,assetsDir:assets()});
    const out=resolve(root,'cism.pdf');
    const rendered=await runner.renderCism({student:'Invented Student',comment:''},out);
    expect(rendered).toEqual({status:'rendered',pages:8,sha256:sha(await readFile(out)),outputPath:out,parityOk:true,parityMismatches:0,parityRawRn:0});
    expect(JSON.parse(await readFile(out+'.env.json','utf8')).argv).toEqual(['render','--type','cism','--assets',assets(),'--out',out]);
    expect(JSON.parse(await readFile(out+'.parity-argv.json','utf8'))).toEqual(['--pdf',out,'--assets',assets(),'--type','cism']);
    // The course_book parity words are unchanged.
    const book=resolve(root,'book-words.pdf');
    await runner.renderCourseBook({student:'Invented Student',comment:''},book);
    expect(JSON.parse(await readFile(book+'.parity-argv.json','utf8'))).toEqual(['--pdf',book,'--assets',assets()]);
    // parity.main exits 1 on a mismatch and still prints its JSON: parityOk is the real result.
    expect(await runner.renderCism({student:'Invented Student',comment:'mismatch'},resolve(root,'cism-mismatch.pdf'))).toMatchObject({status:'rendered',parityOk:false,parityMismatches:2,parityRawRn:1});
    expect(await runner.renderCism({student:'refuse'},resolve(root,'cism-refused.pdf'))).toMatchObject({status:'refused',code:'input_invalid'});
    await expect(runner.renderCism({},'relative.pdf')).rejects.toThrow('output_path_invalid');
  });

  test('studyq_runner_render_report_passes_its_type_to_render_and_parity',async()=>{
    const {src,sourceSha256}=await tree('report-types');
    const runner=pinnedStudyqRunner({interpreter,interpreterSha256,srcDir:src,sourceSha256,assetsDir:assets()});
    for(const type of ['diag','new_diag','formative','ganga','na_score','smart_book','cism'] as const){
      const out=resolve(root,'report-'+type+'.pdf');
      expect(await runner.renderReport(type,{student:'Invented Student',comment:''},out)).toMatchObject({status:'rendered',outputPath:out,parityOk:true});
      expect(JSON.parse(await readFile(out+'.env.json','utf8')).argv).toEqual(['render','--type',type,'--assets',assets(),'--out',out]);
      expect(JSON.parse(await readFile(out+'.parity-argv.json','utf8'))).toEqual(['--pdf',out,'--assets',assets(),'--type',type]);
    }
    const book=resolve(root,'report-book.pdf');
    await runner.renderReport('course_book',{student:'Invented Student',comment:''},book);
    expect(JSON.parse(await readFile(book+'.parity-argv.json','utf8'))).toEqual(['--pdf',book,'--assets',assets()]);
    expect(await runner.renderReport('formative',{student:'Invented Student',comment:'mismatch'},resolve(root,'formative-mismatch.pdf'))).toMatchObject({status:'rendered',parityOk:false,parityMismatches:2,parityRawRn:1});
    expect(await runner.renderReport('diag',{student:'refuse'},resolve(root,'diag-refused.pdf'))).toMatchObject({status:'refused',code:'input_invalid'});
    // A type outside the eight refuses before any spawn; renderCourseBook and renderCism stay for their callers.
    await expect(runner.renderReport('gang_a' as never,{},resolve(root,'x.pdf'))).rejects.toThrow('studyq_report_type_invalid');
    expect(existsSync(resolve(root,'x.pdf.env.json'))).toBe(false);
    expect(typeof runner.renderCourseBook).toBe('function');expect(typeof runner.renderCism).toBe('function');
  });

  test('studyq_runner_refuses_a_changed_or_unsafe_tree_before_any_spawn',async()=>{
    const cases:[string,(src:string)=>Promise<void>][]=[
      ['a changed file',async src=>writeFile(resolve(src,'studyq_linux','cli.py'),cli+'# changed\n')],
      ['an added module',async src=>{await writeFile(resolve(src,'studyq_linux','extra.py'),'');await chmod(resolve(src,'studyq_linux','extra.py'),0o644);}],
      ['a symbolic link',async src=>symlink('/etc/hostname',resolve(src,'studyq_linux','link.py'))],
      ['a __pycache__ directory',async src=>mkdir(resolve(src,'studyq_linux','__pycache__'),{mode:0o755})],
      ['a .pyc file',async src=>{await writeFile(resolve(src,'studyq_linux','cli.cpython-312.pyc'),'');await chmod(resolve(src,'studyq_linux','cli.cpython-312.pyc'),0o644);}],
      ['a group-writable file',async src=>chmod(resolve(src,'studyq_linux','parity.py'),0o664)],
      ['a world-writable directory',async src=>chmod(resolve(src,'studyq_linux'),0o757)],
    ];
    for(const [index,[name,change]] of cases.entries()){
      const {src,sourceSha256}=await tree('unsafe-'+index);
      const runner=pinnedStudyqRunner({interpreter,interpreterSha256,srcDir:src,sourceSha256,assetsDir:assets()});
      await change(src);
      const out=resolve(root,'unsafe-'+index+'.pdf');
      await expect(runner.renderCourseBook({student:'x'},out)).rejects.toThrow('studyq_source_rejected');
      expect({name,spawned:existsSync(out+'.env.json')}).toEqual({name,spawned:false});
    }
    const {src,sourceSha256}=await tree('interpreter');
    const wrong=pinnedStudyqRunner({interpreter,interpreterSha256:'0'.repeat(64),srcDir:src,sourceSha256,assetsDir:assets()});
    await expect(wrong.renderCourseBook({student:'x'},resolve(root,'interpreter.pdf'))).rejects.toThrow('studyq_interpreter_rejected');
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

  // The real executor, only where PyMuPDF and the official templates are installed.
  test.skipIf(process.env.CHECK_STUDYQ_PYMUPDF!=='1'||!process.env.STUDYQ_ASSETS_DIR)('studyq_runner_live_pymupdf_course_book',async()=>{
    const src=resolve(import.meta.dir,'../../../../studyq-linux/src');
    const runner=pinnedStudyqRunner({interpreter,interpreterSha256,srcDir:await realpath(src),sourceSha256:await studyqSourceTreeSha256(src),assetsDir:process.env.STUDYQ_ASSETS_DIR!});
    const report=projectCourseBookReport({block1:[{'1':'Invented book','2':'2040-01-01','3':'2040-03-01','4':'Invented Student'}],block2:[{'1':5,'3':82.5,'4':75,'5':'Invented comment'}],
      block5:[{'1':1,'2':'Invented test','3':10,'4':70,'5':80,'6':'03-01'}],
      bunit_list:[{'1':'Invented chapter','3':80,'4':70,exam_result:[{'1':1,'2':'Invented type','3':2,'4':1,'5':66,'6':'O'}]}],
      block9:[{'2':1,'3':2,'4':3,'5':4}],block10:[{'1':90},{'1':80},{'1':70},{'1':60}],block11:[{'2':71}],block12:[{'2':'Invented unit','3':3,'4':4,'5':70}]},
      {studentName:'Invented Student',studentKey:'770011',courseKey:'560013',bookNo:'430019'});
    const rendered=await runner.renderCourseBook(report.studyqInput,resolve(root,'live.pdf'));
    expect(rendered).toMatchObject({status:'rendered',parityOk:true,parityMismatches:0,parityRawRn:0});
    expect((await readdir(root)).includes('live.pdf')).toBe(true);
  },120000);
});
