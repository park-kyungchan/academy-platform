import {effects, forbiddenFakeDispatch} from './deny-transport.ts';
import {resolve, isAbsolute} from 'node:path';
import {pathToFileURL} from 'node:url';

// No expectations/corpus import: the baseline only receives fixed bytes and input.
const args = process.argv.slice(2);
const option = (name: string) => args[args.indexOf(name) + 1];
const sourceRoot = args.includes('--source-root') ? option('--source-root') : undefined;
const evidenceRoot = args.includes('--evidence-root') ? option('--evidence-root') : undefined;
const control = args.includes('--control') ? option('--control') : undefined;
const RealDate = Date;
globalThis.Date = class extends RealDate {
  constructor(...values: any[]) { super(...(values.length ? values : ['2026-11-02T00:00:00.000Z']) as [any]); }
  static now() { return RealDate.parse('2026-11-02T00:00:00.000Z'); }
} as DateConstructor;

let frame: unknown;
try {
  if (!sourceRoot || !isAbsolute(sourceRoot)) throw Error('s05_explicit_source_root_required');
  if (!evidenceRoot || !isAbsolute(evidenceRoot)) throw Error('s05_explicit_evidence_root_required');
  const input = JSON.parse(await Bun.stdin.text());
  if (control === 'fake-dispatch') forbiddenFakeDispatch();
  if (control === 'network-attempt') { try { await fetch('https://s05.invalid/'); } catch {} }
  if (input.probeTransport) {
    // A guard-only probe, never a retained reference operation or live capture.
    await fetch(input.probeTransport.url, {method: input.probeTransport.method});
    throw Error('s05_guard_failed');
  }
  const {loadSavedContext} = await import(pathToFileURL(resolve(sourceRoot,
    'modules/academy-workbench/tools/academy-mcp/src/saved-context.ts')).href);
  const saved = await loadSavedContext(evidenceRoot);
  frame = {outcome: 'returned', value: saved.query(input.queryInput), effects};
} catch (error) {
  if (!(error instanceof Error)) throw error;
  frame = {outcome: 'threw', error: {name: error.name, message: error.message,
    ...('issues' in error ? {issues: error.issues} : {})}, effects};
}
if (control === 'stdout-junk') process.stdout.write('debug: unexpected\n');
if (control === 'stderr-junk') process.stderr.write('unexpected diagnostic\n');
process.stdout.write(JSON.stringify(frame) + '\n');
if (control === 'nonzero') process.exitCode = 7;
