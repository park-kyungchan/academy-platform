// Migration-only observer of the actual original class. No Rust/candidate import.
import {readFileSync, readdirSync, statSync, mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {DailyRecordService} from '../../modules/academy-workbench/tools/academy-mcp/src/dailyrecord.ts';
import {createDailyRecordWorkflow} from '../../modules/academy-workbench/tools/academy-mcp/src/dailyrecord-workflow.ts';
import {attribution, safeguard, learning, target} from '../workflow-store-reference-v1/fixture.ts';

const [root, caseFile] = process.argv.slice(2);
if (!root || !caseFile || !root.startsWith(process.env.S10_EVIDENCE_ROOT + '/'))
  throw Error('s10_owned_evidence_root_required');
const files = (path: string, prefix = ''): unknown[] => readdirSync(path).sort().flatMap(name => {
  const p = resolve(path, name), key = prefix + name;
  return statSync(p).isDirectory() ? files(p, key + '/') : [{path: key, bytes: readFileSync(p).toString('base64')}];
});
const results = [];
for (const c of JSON.parse(readFileSync(caseFile, 'utf8'))) {
  const dir = resolve(root, c.id);
  mkdirSync(dir, {recursive: true, mode: 0o700});
  const workflow = createDailyRecordWorkflow({storeDir: resolve(dir, 'store'), requiredRoot: dir + '/'});
  const calls: unknown[] = [];
  const backend = {async readDailyRecord(scope: unknown, requestRef: string, subject: unknown) {
    calls.push({port: 'read', scope, requestRef, subject: subject ?? null});
    if ('throw' in c.read) throw Error(c.read.throw);
    return structuredClone(c.read);
  }};
  const policy = {...learning,
    review(scope: unknown, value: string, entries: unknown) {
      calls.push({port: 'learning_review', scope, value, entries: entries ?? null});
      return c.policy.blockers;
    },
    ...('suggestion' in c.policy ? {suggest(scope: unknown, value: string, entries: unknown) {
      calls.push({port: 'learning_suggest', scope, value, entries: entries ?? null});
      return c.policy.suggestion;
    }} : {}),
  };
  const svc = new DailyRecordService(backend as never, safeguard, () => Date.parse(c.now), policy, undefined, workflow);
  if (c.held) workflow.store.publishOnce('observation', 'dailyrecord-observation:s10-regression', {
    observation: c.held, scope: c.storedScope ?? c.input.context.scope, requestRef: c.storedRequest ?? c.input.context.currentRequestRef,
    subject: {displayName: target.studentName}, createdBy: attribution, createdAt: c.now,
  });
  const before = files(resolve(dir, 'store'));
  let output;
  try { output = {result: await svc.review({...c.input, attribution})}; }
  catch (error) { output = {error: (error as Error).message}; }
  const after = files(resolve(dir, 'store'));
  if (JSON.stringify(before) !== JSON.stringify(after)) throw Error('held_review_persisted');
  results.push({id: c.id, ...output, calls, storeUnchanged: true});
}
console.log(JSON.stringify(results));
