import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {effects, forbiddenFakeDispatch} from './isolation.ts';

const args = process.argv.slice(2);
const root = args[args.indexOf('--source-root') + 1];
if (!args.includes('--source-root') || !root?.startsWith('/')) throw Error('s08_explicit_source_root_required');
const load = (file: string) => import(pathToFileURL(resolve(root, file)).href);
const edu = 'apps/academy/dc-gang-a/src/';
const mcp = 'modules/academy-workbench/tools/academy-mcp/src/';
const input = JSON.parse(await Bun.stdin.text());
const events: unknown[] = [];

// JSON declarations construct JS values that cannot survive a JSON wire. These
// tags are adapter inputs, never accepted by the original functions themselves.
function materialize(x: any): any {
  if (!x || typeof x !== 'object') return x;
  if (Array.isArray(x)) return x.map(materialize);
  if (Object.keys(x).length === 1 && '$js' in x) {
    const tag = x.$js;
    if (tag === 'undefined') return undefined;
    if (tag === '-0') return -0;
    if (tag === 'NaN') return NaN;
    if (tag === 'Infinity') return Infinity;
    if (tag === 'bigint') return 1n;
    if (tag === 'function') return () => 1;
    if (tag === 'symbol') return Symbol('synthetic');
    if (tag === 'sparse') return new Array(2);
    if (tag === 'decorated') return Object.assign([1], {extra: 2});
    if (tag === 'date') return new Date('2040-01-01T00:00:00Z');
    if (tag === 'null_prototype') return Object.assign(Object.create(null), {b: 1, a: 2});
    if (tag === 'symbol_key') return {[Symbol('key')]: 1, a: 2};
    if (tag === 'accessor') return Object.defineProperty({}, 'a', {enumerable: true, get() {events.push('accessor_read'); return 1;}});
    if (tag === 'cycle') { const v: any = {}; v.self = v; return v; }
    if (/^depth_\d+$/.test(tag)) {let v: any = null; for (let i = 0; i < Number(tag.slice(6)); i++) v = [v]; return v;}
    throw Error('s08_unknown_js_construction');
  }
  return Object.fromEntries(Object.entries(x).map(([k, v]) => [k, materialize(v)]));
}
const errorValue = (error: any) => ({name: error?.name ?? 'NonError', message: error?.message ?? String(error), ...(error?.issues ? {issues: error.issues} : {})});

async function execute(): Promise<unknown> {
  switch (input.op) {
    case 'width': return (await load(edu + 'lms/lessonJournalTemplates.ts')).journalWireWidth(input.units ? String.fromCharCode(...input.units) : input.value);
    case 'template': return (await load(edu + 'lms/lessonJournalTemplates.ts')).renderLessonJournal(input.value);
    case 'constants': {
      const t = await load(edu + 'lms/lessonJournalTemplates.ts'), c = await load(edu + 'lms/cismWeeklyProtocol.ts');
      return {templates: t.lessonJournalTemplates, activities: t.journalActivities, outcomes: t.journalOutcomes, homeworkActions: t.homeworkActions, cismPath: c.CISM_PATH, cismWireSha256: c.CISM_WIRE_SHA256, narratives: c.CISM_NARRATIVES};
    }
    case 'week': return (await load(edu + 'lms/cismWeeklyProtocol.ts')).cismWeek(input.value);
    case 'month': return (await load(edu + 'lms/cismWeeklyProtocol.ts')).cismMonthDates(input.value);
    case 'json': {
      const value = materialize(input.value);
      if (input.family === 'learning') {
        const m = await load(edu + 'learning/model.ts');
        const bytes = m.canonicalJson(value); return {bytes, sha256: m.digest(value)};
      }
      if (input.family === 'admission') {
        const m = await load(mcp + 'dailyrecord-save-admission.ts');
        const bytes = m.canonicalJson(value); return {bytes, sha256: m.canonicalActionDigest(value)};
      }
      if (input.family === 'cism') {
        const m = await load(edu + 'lms/cismWeeklyProtocol.ts');
        const bytes = JSON.stringify(value); return {bytes, sha256: m.cismHash(value)};
      }
      throw Error('s08_unknown_json_family');
    }
    case 'knowledge': {
      const m = await load(edu + 'learning/model.ts'), value = materialize(input.value);
      m.assertKnowledge(value); return value;
    }
    case 'known': return (await load(edu + 'learning/model.ts')).known(materialize(input.value), input.refs);
    case 'unknown': return (await load(edu + 'learning/model.ts')).unknown(input.reason);
    case 'calendar_date': return (await load(edu + 'learning/model.ts')).parseCalendarDate(input.value);
    case 'review': {
      const {DailyRecordService} = await load(mcp + 'dailyrecord.ts');
      const backend = {async readDailyRecord(scope: unknown, requestRef: string) {
        events.push({event: 'recorded_read', scope, requestRef});
        return input.observation === null ? {status: 'unavailable', reasonCode: 'synthetic_source_unavailable'} : {status: 'observed', observation: structuredClone(input.observation)};
      }};
      const safeguard = input.safeguard === false ? null : {
        policySha256: '3'.repeat(64), safeguardSourceSha256: '4'.repeat(64), sourceRef: 'synthetic-parent-policy',
        checkLines: (s: string) => /^- [^\r\n]+(?:\n- [^\r\n]+)*$/.test(s),
        check: (s: string) => /교사\s*(?:최종\s*)?완료\s*판정/.test(s),
      };
      const policy = input.learning === null ? null : {
        bundleSha256: '5'.repeat(64), configSha256: '6'.repeat(64), catalogSha256: '7'.repeat(64),
        scopeQualified: () => input.learning?.scopeQualified ?? true,
        review: (_scope: unknown, value: string, entries: unknown) => {events.push({event: 'fake_learning_review', value, entries: entries ?? null}); return input.learning?.blockers ?? [];},
      };
      // No workflow store, write backend, lock or journal. Only recorded reads
      // and explicit local policy fakes; they prove service composition only.
      const service = new DailyRecordService(backend, safeguard, () => Date.parse('2040-01-16T09:00:00Z'), policy);
      const intake = await service.intake({context: input.context});
      const review = await service.review({context: input.context, ...(intake.observationHandle ? {observationHandle: intake.observationHandle} : {}), proposals: input.proposals});
      return {intake, review};
    }
    case 'http_action': {
      const m = await load(edu + 'lms/lessonJournalHttp.ts');
      let state = structuredClone(input.state);
      return await m.applyJournalHttpAction(input.action, {
        async read(target: unknown) {events.push({event: 'fake_read', target}); return structuredClone(state);},
        async consumeApproval(action: unknown) {events.push({event: 'fake_approval', action}); return {status: 'verified', actionDigest: m.journalHttpActionDigest(action)};},
        async write(action: any) {events.push({event: 'fake_write', action}); state.fields[action.field] = action.after;},
      });
    }
    default: throw Error('s08_unknown_operation');
  }
}
let outcome: unknown;
try { outcome = {kind: 'return', value: await execute()}; }
catch (error) { outcome = {kind: 'throw', error: errorValue(error)}; }
const frame = {outcome, events, effects};
if (input.control === 'fake_dispatch') forbiddenFakeDispatch();
if (input.control === 'network') {try {await fetch('http://127.0.0.1:1/never');} catch { /* counted */ }}
if (input.control === 'stderr') process.stderr.write('junk\n');
if (input.control === 'stdout') process.stdout.write('junk\n');
process.stdout.write(JSON.stringify(frame) + '\n');
if (input.control === 'nonzero') process.exitCode = 7;
