// Validate a private-students date package: event/student required fields,
// seq order, and manifest sha256/bytes. No dependencies. Usage:
//   node tools/validate.mjs 2026-10-26
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const base = join(dirname(fileURLToPath(import.meta.url)), '..');
const date = process.argv[2];
if (!date) { console.error('usage: node tools/validate.mjs <YYYY-MM-DD>'); process.exit(2); }
const root = join(base, date);
let failures = 0;
const fail = (m) => { failures++; console.error('FAIL', m); };

const EVENT_TYPES = new Set(['attendance_observed','homework_checked','homework_assigned','teacher_decision','assessment_observed','source_observation_failed','reminder_scheduled','interview_pending']);
const EVENT_STATUS = new Set(['confirmed','partially_confirmed','unknown_preserved']);

const eventsPath = join(root, 'events', 'teacher-events.jsonl');
const lines = readFileSync(eventsPath, 'utf8').split('\n').filter(Boolean);
let lastSeq = 0;
const eventIds = new Set();
for (const [i, line] of lines.entries()) {
  let e; try { e = JSON.parse(line); } catch { fail(`events line ${i+1}: invalid JSON`); continue; }
  for (const k of ['schemaVersion','eventId','businessDate','session','seq','type','student','fact','source','sourceRef','status'])
    if (!(k in e)) fail(`events line ${i+1}: missing ${k}`);
  if (e.schemaVersion !== 'academy.context-event/1') fail(`events line ${i+1}: schemaVersion`);
  if (!EVENT_TYPES.has(e.type)) fail(`events line ${i+1}: type ${e.type}`);
  if (!EVENT_STATUS.has(e.status)) fail(`events line ${i+1}: status ${e.status}`);
  if (e.seq !== lastSeq + 1) fail(`events line ${i+1}: seq ${e.seq} after ${lastSeq}`);
  lastSeq = e.seq;
  if (eventIds.has(e.eventId)) fail(`events line ${i+1}: duplicate eventId`);
  eventIds.add(e.eventId);
}

for (const f of readdirSync(join(root, 'students')).filter((x) => x.endsWith('.json'))) {
  const s = JSON.parse(readFileSync(join(root, 'students', f), 'utf8'));
  for (const k of ['schemaVersion','student','businessDate','derivedFrom','attendance','books','nextHomework','unknowns'])
    if (!(k in s)) fail(`students/${f}: missing ${k}`);
  for (const ref of s.derivedFrom ?? []) {
    const id = ref.split('#')[1];
    if (id && !eventIds.has(id)) fail(`students/${f}: derivedFrom unknown event ${id}`);
  }
}

const manifest = JSON.parse(readFileSync(join(root, 'manifest.json'), 'utf8'));
for (const a of manifest.artifacts) {
  const p = join(root, a.path);
  const b = readFileSync(p);
  const h = createHash('sha256').update(b).digest('hex');
  if (h !== a.sha256) fail(`manifest hash mismatch: ${a.path}`);
  if (b.length !== a.bytes) fail(`manifest bytes mismatch: ${a.path}`);
  if (statSync(p).size !== a.bytes) fail(`manifest size mismatch: ${a.path}`);
}

if (failures) { console.error(`validate ${date}: ${failures} failure(s)`); process.exit(1); }
console.log(`validate ${date}: OK (${lines.length} events, ${manifest.artifacts.length} artifacts)`);
