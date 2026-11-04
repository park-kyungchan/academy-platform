import {readFileSync, writeFileSync, mkdirSync, mkdtempSync, symlinkSync, copyFileSync, realpathSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {resolve, dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {tmpdir} from 'node:os';
import {compare} from './comparator.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const sourceRoot = resolve(here, '../..');
const args = process.argv.slice(2);
const arg = name => args.includes(name) ? args[args.indexOf(name) + 1] : undefined;
const depsRoot = arg('--deps-root');
const output = arg('--results-root');
if (!depsRoot || !output) throw Error('usage: node verify.mjs --deps-root EXISTING_NODE_MODULES --results-root NEW_LOCAL_RESULTS');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const json = path => JSON.parse(readFileSync(path, 'utf8'));
const manifest = json(join(here, 'pins.json'));
for (const [path, expected] of Object.entries(manifest.files)) {
  if (hash(readFileSync(resolve(sourceRoot, path))) !== expected) throw Error('s05_frozen_input_changed:' + path);
}
if (json(join(depsRoot, 'zod/package.json')).version !== '4.4.3') throw Error('s05_existing_zod_version_required');
// No dependency installation. Resolve only the already installed package.
const zodRoot = realpathSync(join(depsRoot, 'zod'));
for (const [path, expected] of Object.entries(manifest.zodFiles)) {
  if (hash(readFileSync(join(zodRoot, path))) !== expected) throw Error('s05_zod_bytes_changed:' + path);
}
mkdirSync(output, {recursive: true});
for (const name of ['reference.json', 'qualification.json', 'source-test.json', 'report.json']) {
  try { readFileSync(join(output, name)); throw Error('s05_result_already_exists:' + name); }
  catch (error) { if (error.code !== 'ENOENT') throw error; }
}
const temp = mkdtempSync(join(tmpdir(), 's05-recorded-lookup-'));
const env = {PATH: process.env.PATH, NODE_PATH: resolve(depsRoot), BUN_CONFIG_NO_AUTO_INSTALL: '1',
  HOME: join(temp, 'empty-home'), TMPDIR: temp, TZ: 'UTC'};
mkdirSync(env.HOME);
const run = (cmd, argv, input = '') => {
  const result = spawnSync(cmd, argv, {input, encoding: 'utf8', cwd: temp, env,
    timeout: 10000, maxBuffer: 2 * 1024 * 1024, killSignal: 'SIGKILL'});
  return {command: [cmd, ...argv], cwd: temp, status: result.status, signal: result.signal,
    spawnError: result.error ? {code: result.error.code, message: result.error.message} : null,
    stdout: result.stdout ?? '', stderr: result.stderr ?? ''};
};
const cases = json(join(here, 'corpus/cases.json'));
const expected = json(join(here, 'expected.json'));
if (Object.keys(expected).join('\0') !== cases.map(c => c.id).join('\0')) throw Error('s05_case_inventory_mismatch');
const auditPath = 'private/student-audit-20261014/audit.json';
const lessonPath = 'private/lesson-20261014/overlay.json';
const pinPath = 'tools/academy-mcp/architecture/source-pins.json';
function stage(testCase, suffix = '') {
  const root = join(temp, testCase.id + suffix);
  for (const [target, fixture] of [[auditPath, 'audit.json'], [lessonPath, 'lesson.json'], [pinPath, 'source-pins.json']]) {
    mkdirSync(dirname(join(root, target)), {recursive: true});
    copyFileSync(join(here, 'corpus', fixture), join(root, target));
  }
  const audit = json(join(root, auditPath)), lesson = json(join(root, lessonPath)), pins = json(join(root, pinPath));
  if (testCase.variant === 'changed-bytes') writeFileSync(join(root, auditPath), readFileSync(join(root, auditPath), 'utf8') + ' ');
  if (testCase.variant === 'missing-pin') pins.files = pins.files.slice(1);
  if (testCase.variant === 'duplicate-student-id') {
    audit.students[1].studentId = audit.students[0].studentId;
    writeFileSync(join(root, auditPath), JSON.stringify(audit));
    pins.files[0].sha256 = hash(readFileSync(join(root, auditPath)));
  }
  if (testCase.variant === 'foreign-lesson-student') {
    lesson.students[0].studentId = 'not-preserved';
    writeFileSync(join(root, lessonPath), JSON.stringify(lesson));
    pins.files[1].sha256 = hash(readFileSync(join(root, lessonPath)));
  }
  if (testCase.variant === 'symlink-input') {
    // Keep originals; create a separate root whose audit leaf is a symlink.
    const linkRoot = root + '-link';
    for (const path of [auditPath, lessonPath, pinPath]) mkdirSync(dirname(join(linkRoot, path)), {recursive: true});
    symlinkSync(join(root, auditPath), join(linkRoot, auditPath));
    copyFileSync(join(root, lessonPath), join(linkRoot, lessonPath));
    copyFileSync(join(root, pinPath), join(linkRoot, pinPath));
    return linkRoot;
  }
  writeFileSync(join(root, pinPath), JSON.stringify(pins));
  return root;
}
const baseline = join(here, 'baseline.ts');
function invoke(c, evidenceRoot, control, script = baseline) {
  const argv = ['--no-install', script];
  if (!c.omitSourceRoot) argv.push('--source-root', sourceRoot);
  if (!c.omitEvidenceRoot) argv.push('--evidence-root', evidenceRoot);
  if (control) argv.push('--control', control);
  return run('bun', argv, JSON.stringify(c.input));
}
const references = cases.map(c => {
  const evidenceRoot = stage(c);
  const actual = invoke(c, evidenceRoot);
  return {id: c.id, evidenceRoot, input: c.input, actual, comparison: compare(expected[c.id], actual)};
});
// Relocate executable and evidence independently; the source root stays explicit.
const relocatedScript = join(temp, 'independent-executable/baseline.ts');
mkdirSync(dirname(relocatedScript));
copyFileSync(baseline, relocatedScript);
copyFileSync(join(here, 'deny-transport.ts'), join(dirname(relocatedScript), 'deny-transport.ts'));
const first = cases[0], movedRoot = stage(first, '-relocated');
const moved = invoke(first, movedRoot, undefined, relocatedScript);
references.push({id: 'relocated-executable-and-evidence', evidenceRoot: movedRoot, input: first.input,
  actual: moved, comparison: compare(expected[first.id], moved)});

const qualifications = [];
const baseActual = references[0].actual;
const mutate = (id, change) => {
  const frame = structuredClone(expected[first.id]); change(frame);
  const actual = {...baseActual, stdout: JSON.stringify(frame) + '\n'};
  qualifications.push({id, actual, comparison: compare(expected[first.id], actual)});
};
mutate('wrong-book-value', frame => { frame.value.records[0].value.displayName = 'Wrong book'; });
mutate('missing-required-field', frame => { delete frame.value.absenceProvesNonperformance; });
mutate('completion-inferred', frame => { frame.value.absenceProvesNonperformance = true; });
mutate('wrong-provenance', frame => { frame.value.records[0].source.jsonPointer = '/students/1/books/0'; });
mutate('wrong-source-digest', frame => { frame.value.snapshot.sha256 = '0'.repeat(64); });
mutate('unknown-becomes-complete', frame => { frame.value.scope.completeTeacherRosterVerified = true; });
mutate('cursor-identity-changed', frame => { frame.value.page.nextCursor = 'wrong'; });
for (const [id, caseId, change] of [
  ['null-becomes-missing', 'attempt-knowledge-states', frame => { delete frame.value.records[2].value.score; }],
  ['blank-becomes-zero', 'attempt-knowledge-states', frame => { frame.value.records[3].value.score = 0; }],
  ['unknown-becomes-zero', 'attempt-knowledge-states', frame => { frame.value.records[5].value.score = 0; }],
  ['record-order-reversed', 'sections-order', frame => { frame.value.records.reverse(); }],
  ['wrong-refusal-code', 'unknown-student', frame => { frame.value.code = 'not_found'; }],
  ['refusal-becomes-empty-success', 'unknown-student', frame => { frame.value.status = 'partial'; }],
]) {
  const original = references.find(r => r.id === caseId).actual;
  const frame = structuredClone(expected[caseId]); change(frame);
  const actual = {...original, stdout: JSON.stringify(frame) + '\n'};
  qualifications.push({id, actual, comparison: compare(expected[caseId], actual)});
}
// Real child controls: a plausible stdout never substitutes for actual status/effects.
for (const control of ['nonzero', 'stdout-junk', 'stderr-junk', 'fake-dispatch', 'network-attempt']) {
  const actual = invoke(first, references[0].evidenceRoot, control);
  qualifications.push({id: control, actual, comparison: compare(expected[first.id], actual)});
}
// Bun's test resolver ignores NODE_PATH here. Mirror only the hash-checked pure
// closure and the two unchanged source-test files into disposable storage.
// The reference runs above still import directly from the assigned checkout.
const testRoot = join(temp, 'source-test-copy');
const testPath = 'modules/academy-workbench/tools/academy-mcp/tests/saved-context.test.ts';
const fixturePath = 'modules/academy-workbench/tools/academy-mcp/tests/fixtures.ts';
for (const path of [...manifest.sourceClosure, testPath, fixturePath]) {
  mkdirSync(dirname(join(testRoot, path)), {recursive: true});
  copyFileSync(join(sourceRoot, path), join(testRoot, path));
}
symlinkSync(resolve(depsRoot), join(testRoot, 'node_modules'));
// Run one existing synthetic test boundary; never package/integration scripts.
const sourceTest = run('bun', ['test', '--no-install', '--preload', join(here, 'deny-transport.ts'),
  join(testRoot, testPath)]);
const report = {
  schema: 's05.recorded-lookup-check/1', sourceBase: manifest.sourceBase,
  sourceRoot, depsRoot: resolve(depsRoot), tempEvidenceRoot: temp,
  runtime: {bun: run('bun', ['--version']), node: process.version, zod: '4.4.3'},
  frozenPinsSha256: hash(readFileSync(join(here, 'pins.json'))),
  expectedSha256: hash(readFileSync(join(here, 'expected.json'))),
  reference: {total: references.length, accepted: references.filter(r => r.comparison.accepted).length},
  qualification: {total: qualifications.length, rejected: qualifications.filter(r => !r.comparison.accepted).length},
  sourceTestStatus: sourceTest.status, childrenRunningAtReturn: 0,
};
for (const [name, value] of [['reference.json', references], ['qualification.json', qualifications], ['source-test.json', sourceTest], ['report.json', report]])
  writeFileSync(join(output, name), JSON.stringify(value, null, 2) + '\n', {flag: 'wx'});
const ok = report.reference.accepted === report.reference.total && report.qualification.rejected === report.qualification.total
  && sourceTest.status === 0 && sourceTest.signal === null && sourceTest.spawnError === null;
process.stdout.write(JSON.stringify({ok, ...report.reference, negativeControlsRejected: report.qualification.rejected,
  sourceTestStatus: report.sourceTestStatus, rawResults: resolve(output)}) + '\n');
process.exitCode = ok ? 0 : 1;
