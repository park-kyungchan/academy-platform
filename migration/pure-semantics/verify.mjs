import {readFileSync, writeFileSync, mkdirSync, mkdtempSync, symlinkSync, copyFileSync, realpathSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {resolve, dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {tmpdir} from 'node:os';
import assert from 'node:assert/strict';
import {compare} from './comparator.mjs';
const here = dirname(fileURLToPath(import.meta.url)), root = resolve(here,'../..');
const args = process.argv.slice(2), arg = name => args[args.indexOf(name)+1];
if (!args.includes('--deps-root') || !args.includes('--results-root')) throw Error('usage: node verify.mjs --deps-root EXISTING_NODE_MODULES --results-root NEW_DIRECTORY [--freeze-reference]');
const deps = realpathSync(arg('--deps-root')), output = resolve(arg('--results-root'));
const sha = value => createHash('sha256').update(value).digest('hex');
const json = path => JSON.parse(readFileSync(path,'utf8'));
const pins = json(join(here,'pins.json')), cases = json(join(here,'corpus/cases.json'));
const expectedPath = join(here,'expected-v2.json');
if (!args.includes('--freeze-reference')) {
  const frozen = json(join(here,'freeze.json'));
  for (const [path,digest] of Object.entries(frozen.files)) assert.equal(sha(readFileSync(join(root,path))),digest,'frozen:'+path);
}
assert.equal(sha(readFileSync(join(here,'corpus/cases.json'))),pins.corpusSha256);
for (const [path,digest] of Object.entries(pins.sourceClosure)) assert.equal(sha(readFileSync(join(root,path))),digest,path);
const zodRoot = realpathSync(join(deps,'zod'));
assert.equal(json(join(zodRoot,'package.json')).version,pins.zod);
for (const [path,digest] of Object.entries(pins.zodFiles)) assert.equal(sha(readFileSync(join(zodRoot,path))),digest,path);
// A new result directory is required. Every raw child result is retained.
mkdirSync(output);
const temp = mkdtempSync(join(tmpdir(),'s08-pure-')), sourceRoot = join(temp,'source');
for (const path of Object.keys(pins.sourceClosure)) {mkdirSync(dirname(join(sourceRoot,path)),{recursive:true});copyFileSync(join(root,path),join(sourceRoot,path));}
mkdirSync(join(sourceRoot,'node_modules')); symlinkSync(zodRoot,join(sourceRoot,'node_modules/zod'));
mkdirSync(join(temp,'home'));
const env = {PATH:process.env.PATH,HOME:join(temp,'home'),TMPDIR:temp,TZ:'UTC',BUN_CONFIG_NO_AUTO_INSTALL:'1'};
function run(cmd, argv, input='') {
  const r = spawnSync(cmd,argv,{input,encoding:'utf8',cwd:temp,env,timeout:10000,maxBuffer:4*1024*1024,killSignal:'SIGKILL'});
  return {command:[cmd,...argv],cwd:temp,status:r.status,signal:r.signal,spawnError:r.error?{code:r.error.code,message:r.error.message}:null,stdout:r.stdout??'',stderr:r.stderr??''};
}
const bunVersion = run('bun',['--version']);
assert.equal(bunVersion.status,0); assert.equal(bunVersion.stdout.trim(),pins.bun); assert.equal(process.version,pins.node);
const invoke = (input, script=join(here,'baseline.ts')) => run('bun',['--no-env-file','--no-install',script,'--source-root',sourceRoot],JSON.stringify(input));
const raw = cases.map(c=>({id:c.id,input:c.input,actual:invoke(c.input)}));
writeFileSync(join(output,'reference.json'),JSON.stringify(raw,null,2)+'\n',{flag:'wx'});
const freeze = args.includes('--freeze-reference');
let expected;
if (freeze) {
  expected = Object.fromEntries(raw.map(r=>{
    const parsed = JSON.parse(r.actual.stdout);
    assert.equal(compare(parsed,r.actual).accepted,true,r.id);
    return [r.id,parsed];
  }));
  writeFileSync(expectedPath,JSON.stringify(expected,null,2)+'\n',{flag:'wx'});
} else expected = json(expectedPath);
assert.deepEqual(Object.keys(expected),cases.map(c=>c.id));
const comparisons = raw.map(r=>({id:r.id,...compare(expected[r.id],r.actual)}));
// Independent literal claims prevent a source loader failure from being frozen
// as successful characterization. These check the actual source result shape.
const v = id => expected[id].outcome.value;
const literalChecks = [];
function check(name, fn) {try{fn();literalChecks.push({name,passed:true});}catch(error){literalChecks.push({name,passed:false,error:error.message});}}
check('utf16-and-normalization',()=>{assert.equal(v('width-emoji'),4);assert.equal(v('width-composed'),2);assert.equal(v('width-decomposed'),3);assert.equal(v('width-high'),2);});
check('calendar-intersection',()=>{assert.deepEqual(v('week-2026-01-01'),{periodStart:'2025-12-28',periodEnd:'2026-01-03'});assert.deepEqual(v('month-2026-11'),['2026-11-01','2026-11-08','2026-11-15','2026-11-22','2026-11-29']);});
check('previous-physical-record',()=>{const r=v('review-physical-current-and-previous').review;assert.equal(r.status,'prepared');assert.deepEqual(r.patch.map(p=>p.target.recordId),['current-record','previous-record']);});
check('independent-field-patch',()=>{const r=v('review-learning-hold-independent-assessment').review;assert.equal(r.status,'held');assert.equal(r.rows[0].disposition,'blocked');assert.deepEqual(r.patch.map(p=>p.field),['attendance']);});
check('narrative-explicit-policy',()=>{assert.ok(v('review-narrative-set').review.rows[0].blockers.includes('existing_value_requires_explicit_replace'));assert.equal(v('review-narrative-replace').review.rows[0].disposition,'proposed');assert.equal(v('review-narrative-noop-set').review.rows[0].disposition,'noop');assert.ok(v('review-memo-no-addition-clear-hold').review.rows[0].blockers.includes('no_addition_preserves_existing_memo'));});
check('knowledge-missing-zero',()=>{assert.equal(expected['knowledge-missing'].outcome.kind,'throw');assert.equal(v('knowledge-zero').value,0);assert.equal(v('knowledge-blank').value,'');assert.equal(v('knowledge-not_performed').value,'not_performed');});
check('digest-families-not-collapsed',()=>{assert.notEqual(v('json-learning-ordering').bytes,v('json-cism-ordering').bytes);assert.equal(expected['json-learning-undefinedMember'].outcome.kind,'throw');assert.equal(v('json-admission-undefinedMember').bytes,'{"b":null}');});
check('actual-http-fake-target-selection',()=>{assert.equal(v('http-previousHomeworkAssessment-true').status,'persisted');assert.equal(expected['http-previousHomeworkAssessment-false'].outcome.error.message,'journal_http_effect_record_conflict');});
check('deterministic-handle-retained',()=>{assert.equal(v('review-physical-current-and-previous').intake.observationHandle,'dailyrecord-observation:00000000-0000-4000-8000-000000000001');});
const controls = [];
const baseCase = cases.find(c=>c.id==='width-emoji'), exp = expected[baseCase.id];
for (const control of ['nonzero','stdout','stderr','fake_dispatch','network']) {const actual=invoke({...baseCase.input,control});controls.push({id:control,actual,comparison:compare(exp,actual)});}
// Mutants run as ACTUAL children too; no fabricated child status. Input is a
// deliberately changed full reference frame, emitted by a minimal process.
const mutantScript = 'process.stdout.write(JSON.stringify(JSON.parse(require("node:fs").readFileSync(0,"utf8")))+"\\n")';
function mutant(id,expectedFrame,fn) {const value=structuredClone(expectedFrame);fn(value);const actual=run('node',['-e',mutantScript],JSON.stringify(value));controls.push({id,actual,comparison:compare(expectedFrame,actual)});}
mutant('wrong-width',exp,x=>x.outcome.value=2);
mutant('missing-field',exp,x=>delete x.effects);
mutant('wrong-error',expected['knowledge-missing'],x=>x.outcome.error.message='generic_failure');
mutant('digest-bytes',expected['json-learning-ordering'],x=>x.outcome.value.bytes+=' ');
mutant('digest-only',expected['json-learning-ordering'],x=>x.outcome.value.sha256='0'.repeat(64));
mutant('ordered-array',expected['month-2026-10'],x=>x.outcome.value.reverse());
mutant('null-to-missing',expected['review-physical-current-and-previous'],x=>delete x.outcome.value.review.rows[0].learningEntries);
mutant('knowledge-zero-to-blank',expected['knowledge-zero'],x=>x.outcome.value.value='');
mutant('physical-record',expected['review-physical-current-and-previous'],x=>x.outcome.value.review.patch[1].target.recordId='current-record');
mutant('field-hold',expected['review-learning-hold-independent-assessment'],x=>x.outcome.value.review.patch=[]);
mutant('event-order',expected['http-previousHomeworkAssessment-true'],x=>x.events.reverse());
const whitespace = run('node',['-e','process.stdout.write(" "+require("node:fs").readFileSync(0,"utf8"))'],JSON.stringify(exp)+'\n');
controls.push({id:'protocol-whitespace',actual:whitespace,comparison:compare(exp,whitespace)});
writeFileSync(join(output,'qualification.json'),JSON.stringify(controls,null,2)+'\n',{flag:'wx'});
const test = run('bun',['--no-env-file','--no-install','test',join(sourceRoot,'apps/academy/dc-gang-a/tests/lms/lessonJournalTemplates.test.ts')]);
writeFileSync(join(output,'source-test.json'),JSON.stringify(test,null,2)+'\n',{flag:'wx'});
const report = {schema:'s08.reference-result/1',foundationOnly:true,independentReview:'pending',rustSemanticLeaf:'unimplemented',rustCompilation:'not_run_no_build_slot',sourceRoot,temp,bunVersion,expectedPath,expectedSha256:sha(readFileSync(expectedPath)),cases:comparisons.length,matched:comparisons.filter(x=>x.accepted).length,leafCases:cases.filter(c=>c.leaf).length,negativeControls:controls.length,rejected:controls.filter(x=>!x.comparison.accepted).length,literalChecks,comparisons,sourceTestStatus:test.status,sourceTestSignal:test.signal,sourceTestSpawnError:test.spawnError};
report.passed=report.matched===report.cases&&report.rejected===report.negativeControls&&literalChecks.every(x=>x.passed)&&test.status===0&&test.signal===null&&test.spawnError===null;
writeFileSync(join(output,'report.json'),JSON.stringify(report,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({passed:report.passed,cases:report.cases,matched:report.matched,negativeControls:report.negativeControls,rejected:report.rejected,literalChecks:literalChecks.filter(x=>x.passed).length,sourceTestStatus:test.status,results:output}));
if(!report.passed) process.exitCode=1;
