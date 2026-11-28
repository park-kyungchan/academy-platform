import {createHash} from 'node:crypto';
import {closeSync,constants,fstatSync,fsyncSync,lstatSync,openSync,readFileSync,statSync,unlinkSync,writeSync} from 'node:fs';
import {dirname,isAbsolute,resolve,sep} from 'node:path';

/**
 * The attempt ledger and the review-token engine of the gated Academy writes (specs/academy-mcp/gated-writes.md
 * R2 to R5, R10 to R16). One private JSON-lines file of the server's owner (mode 0600, outside the repository);
 * every record is appended, written and synced to disk before the request it guards is sent, so a record
 * outlives the stateless per-call endpoint process that wrote it. Records hold keys and hashes only: never a
 * student name, record text, cookie or password (R16). The clinic tools use it (clinicPaperWrite.ts); the
 * FA and unit-exam tools of `ama-gated-write-exams` reuse it with their own `tool` family name.
 */

/** A refusal with one public code of the table "Codes" of the specification; the message is the code. */
export class GatedWriteRefusal extends Error {
  constructor(readonly code: string) { super(code); this.name = 'GatedWriteRefusal'; }
}

/** Life of a `reviewToken` (R2, R5). */
export const REVIEW_TOKEN_TTL_MS = 15 * 60 * 1000;
/** A lock file that cannot be read is treated as being written by a live call for this long (ms). */
const LOCK_WRITE_GRACE_MS = 5000;
const SHA256 = /^[a-f0-9]{64}$/;

export const sha256Hex = (value: string | Uint8Array): string => createHash('sha256').update(value).digest('hex');

/** JSON with object members in sorted order and no whitespace, so equal values give equal text. */
export function canonicalJson(value: unknown): string {
  if (value === null || typeof value !== 'object') {
    const text = JSON.stringify(value);
    if (text === undefined) throw new Error('canonical_json_unsupported');
    return text;
  }
  if (Array.isArray(value)) return '[' + value.map(canonicalJson).join(',') + ']';
  const record = value as Record<string, unknown>;
  return '{' + Object.keys(record).filter(k => record[k] !== undefined).sort().map(k => JSON.stringify(k) + ':' + canonicalJson(record[k])).join(',') + '}';
}

export const changeSha256Of = (change: unknown): string => sha256Hex(canonicalJson(change));

/** The ledger family of the FA and unit-exam tools: records of this tool have no single student (`studentKey` is absent) and keep `targets`. */
export const EXAM_LEDGER_TOOL = 'academy_exam_paper';
/**
 * `studentKey` is the one student of a clinic record; an exam record has none and names its sorted `targets` instead
 * (the strings `iden!priNo!gradeNo!classNo`, keys only), which enter the canonical token JSON in its place (R2).
 */
export interface ReviewTokenInput {tool: string; change: unknown; studentKey?: string; targets?: readonly string[]; scopeKey: string; wireSha256: string; expiresAt: string}
/** R2: the SHA-256 of the canonical JSON of the change, the student (or target) and scope keys, the wire hash and the expiry. */
export const reviewTokenOf = (input: ReviewTokenInput): string => sha256Hex(canonicalJson({
  tool: input.tool, change: input.change, studentKey: input.studentKey, targets: input.targets, scopeKey: input.scopeKey, wireSha256: input.wireSha256, expiresAt: input.expiresAt}));

export type AttemptStep = 'before_write' | 'write_dispatched' | 'response_received' | 'reconciled';
export type AttemptState = 'open' | 'persisted' | 'partially_persisted' | 'absent';
/** The three steps of an exam save (specs/academy-mcp/gated-writes.md R8): A stages the items, B saves the paper, C assigns it. */
export type ExamStep = 'A' | 'B' | 'C';
export interface ReviewedRecord {
  kind: 'reviewed'; tool: string; reviewToken: string; changeSha256: string; studentKey?: string; targets?: string[]; scopeKey: string;
  wireSha256: string; reviewedAt: string; expiresAt: string;
}
export interface AttemptRecord {
  kind: 'attempt'; attemptId: string; tool: string; reviewToken: string; changeSha256: string; studentKey?: string; targets?: string[]; scopeKey: string;
  step: AttemptStep; examStep?: ExamStep; testno?: string; responseSha256?: string; paperKeys?: string[]; state: AttemptState; at: string; runId: string;
}
export type LedgerRecord = ReviewedRecord | AttemptRecord;

const KEY = /^[1-9][0-9]{0,14}$/;
const STEPS = new Set<string>(['before_write', 'write_dispatched', 'response_received', 'reconciled']);
const STATES = new Set<string>(['open', 'persisted', 'partially_persisted', 'absent']);
const EXAM_STEPS = new Set<string>(['A', 'B', 'C']);
/** One target of an exam record: `iden!priNo!gradeNo!classNo`, keys only (R16). */
export const TARGET_STRING = /^[1-3]![1-9][0-9]{0,14}![0-9]{1,15}![0-9]{1,15}$/;
const text = (v: unknown, max = 200): v is string => typeof v === 'string' && v.length > 0 && v.length <= max;
/** The student or targets of a record: a clinic record has a numeric `studentKey` and no targets; an exam record has no `studentKey` and 1 to 40 sorted distinct targets. */
function validSubject(r: Record<string, unknown>): boolean {
  if (r.tool === EXAM_LEDGER_TOOL) {
    return r.studentKey === undefined && Array.isArray(r.targets) && r.targets.length >= 1 && r.targets.length <= 40
      && r.targets.every((t, i, all) => typeof t === 'string' && TARGET_STRING.test(t) && (i === 0 || (all[i - 1] as string) < t));
  }
  return typeof r.studentKey === 'string' && KEY.test(r.studentKey) && r.targets === undefined;
}
function validRecord(value: unknown): value is LedgerRecord {
  if (value === null || typeof value !== 'object') return false;
  const r = value as Record<string, unknown>;
  if (r.kind === 'reviewed') {
    return text(r.tool) && typeof r.reviewToken === 'string' && SHA256.test(r.reviewToken) && typeof r.changeSha256 === 'string' && SHA256.test(r.changeSha256)
      && validSubject(r) && r.examStep === undefined && r.testno === undefined && text(r.scopeKey) && typeof r.wireSha256 === 'string' && SHA256.test(r.wireSha256)
      && text(r.reviewedAt, 40) && text(r.expiresAt, 40);
  }
  if (r.kind === 'attempt') {
    const exam = r.tool === EXAM_LEDGER_TOOL;
    return text(r.attemptId, 80) && text(r.tool) && typeof r.reviewToken === 'string' && SHA256.test(r.reviewToken) && typeof r.changeSha256 === 'string' && SHA256.test(r.changeSha256)
      && validSubject(r) && text(r.scopeKey) && typeof r.step === 'string' && STEPS.has(r.step)
      && typeof r.state === 'string' && STATES.has(r.state) && text(r.at, 40) && text(r.runId, 80)
      && (exam ? typeof r.examStep === 'string' && EXAM_STEPS.has(r.examStep) && (r.testno === undefined || (typeof r.testno === 'string' && KEY.test(r.testno)))
        : r.examStep === undefined && r.testno === undefined)
      && (r.responseSha256 === undefined || (typeof r.responseSha256 === 'string' && SHA256.test(r.responseSha256)))
      && (r.paperKeys === undefined || (Array.isArray(r.paperKeys) && r.paperKeys.length <= 200 && r.paperKeys.every(k => typeof k === 'string' && KEY.test(k))));
  }
  return false;
}

/** The open (unresolved) attempts: those whose latest record has state `open`, with that record. */
export function latestAttempts(records: readonly LedgerRecord[]): Map<string, AttemptRecord> {
  const latest = new Map<string, AttemptRecord>();
  for (const r of records) if (r.kind === 'attempt') latest.set(r.attemptId, r);
  return latest;
}
/**
 * `scopePrefix` and `targets` are the exam form of R11: an attempt of the same family (its `scopeKey` starts with the prefix) whose kept
 * `targets` intersect `targets`, tested on the kept lists and not on hashes.
 */
export function openAttempts(records: readonly LedgerRecord[], where: {tool?: string; studentKey?: string; scopeKey?: string; runId?: string; scopePrefix?: string; targets?: readonly string[]} = {}): AttemptRecord[] {
  return [...latestAttempts(records).values()].filter(a => a.state === 'open'
    && (where.tool === undefined || a.tool === where.tool) && (where.studentKey === undefined || a.studentKey === where.studentKey)
    && (where.scopeKey === undefined || a.scopeKey === where.scopeKey)
    && (where.scopePrefix === undefined || a.scopeKey.startsWith(where.scopePrefix))
    && (where.targets === undefined || (a.targets ?? []).some(t => where.targets!.includes(t))));
}
/** The steps recorded for one attempt, in the order written. */
export const stepsOf = (records: readonly LedgerRecord[], attemptId: string): AttemptRecord[] => records.filter((r): r is AttemptRecord => r.kind === 'attempt' && r.attemptId === attemptId);

/**
 * R4, R5: the reviewed record of `token` for `tool`, or the refusal. A token that is unknown, already used
 * (any attempt carries it), made by another tool, whose change hashes differently, or that does not
 * recompute from its record gives `review_token_invalid`; a matching token older than its expiry gives
 * `review_token_expired`.
 */
export function verifyReviewToken(records: readonly LedgerRecord[], input: {tool: string; token: string; change: unknown; studentKey?: string; targets?: readonly string[]; scopeKey: string; now: number}): ReviewedRecord {
  const reviewed = records.find((r): r is ReviewedRecord => r.kind === 'reviewed' && r.reviewToken === input.token);
  if (!reviewed || reviewed.tool !== input.tool) throw new GatedWriteRefusal('review_token_invalid');
  if (records.some(r => r.kind === 'attempt' && r.reviewToken === input.token)) throw new GatedWriteRefusal('review_token_invalid');
  if (reviewed.changeSha256 !== changeSha256Of(input.change) || reviewed.studentKey !== input.studentKey || reviewed.scopeKey !== input.scopeKey
    || JSON.stringify(reviewed.targets ?? null) !== JSON.stringify(input.targets ?? null)) throw new GatedWriteRefusal('review_token_invalid');
  const recomputed = reviewTokenOf({tool: reviewed.tool, change: input.change, studentKey: reviewed.studentKey, targets: reviewed.targets, scopeKey: reviewed.scopeKey, wireSha256: reviewed.wireSha256, expiresAt: reviewed.expiresAt});
  if (recomputed !== input.token) throw new GatedWriteRefusal('review_token_invalid');
  const expires = Date.parse(reviewed.expiresAt);
  if (!Number.isFinite(expires) || input.now > expires) throw new GatedWriteRefusal('review_token_expired');
  return reviewed;
}

function fsyncDirectory(path: string): void {
  const fd = openSync(path, constants.O_RDONLY);
  try { fsyncSync(fd); } finally { closeSync(fd); }
}
const repositoryRoot = resolve(import.meta.dir, '../../../../..');

/** The lock held while one call works on the ledger: `<ledger>.lock`, created exclusively (R11). */
export interface LedgerLock {release(): void; staleRunId: string | null | undefined}

export class GatedWriteLedger {
  readonly path: string;
  readonly lockPath: string;
  /** `path` is the configured private ledger file; undefined, empty or relative is refused on first use (`ledger_unavailable`). */
  constructor(path: string | null | undefined, private readonly pidAlive: (pid: number) => boolean = defaultPidAlive) {
    this.path = typeof path === 'string' ? path : '';
    this.lockPath = this.path + '.lock';
  }

  /**
   * R10: refuse with `ledger_unavailable`, before any HTTP request, unless the path is set, absolute and
   * normalized, outside the repository, not a symlink, a regular file only its owner can read or write
   * (created with mode 0600 when absent) and writable, in a directory no other user can write.
   */
  assertAvailable(): void {
    const fail = (): never => { throw new GatedWriteRefusal('ledger_unavailable'); };
    const path = this.path;
    if (!path || !isAbsolute(path) || resolve(path) !== path || path.includes('\0')) fail();
    if (path === repositoryRoot || path.startsWith(repositoryRoot + sep)) fail();
    try {
      const directory = lstatSync(dirname(path));
      if (!directory.isDirectory() || directory.isSymbolicLink() || (directory.mode & 0o022) !== 0 || directory.uid !== process.getuid?.()) fail();
      let existing = null;
      try { existing = lstatSync(path); } catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error; }
      if (existing === null) {
        const fd = openSync(path, constants.O_WRONLY | constants.O_APPEND | constants.O_CREAT | constants.O_EXCL | constants.O_NOFOLLOW, 0o600);
        try { fsyncSync(fd); } finally { closeSync(fd); }
        fsyncDirectory(dirname(path));
      }
      const fd = openSync(path, constants.O_WRONLY | constants.O_APPEND | constants.O_NOFOLLOW);
      try {
        const s = fstatSync(fd);
        if (!s.isFile() || s.nlink !== 1 || (s.mode & 0o077) !== 0 || s.uid !== process.getuid?.()) fail();
      } finally { closeSync(fd); }
    } catch (error) {
      if (error instanceof GatedWriteRefusal) throw error;
      fail();
    }
  }

  /** R10: one record, written and synced to disk before this returns; a record that cannot be made is `ledger_unavailable`. */
  append(record: LedgerRecord): void {
    this.assertAvailable();
    if (!validRecord(record)) throw new GatedWriteRefusal('ledger_unavailable');
    try {
      const fd = openSync(this.path, constants.O_WRONLY | constants.O_APPEND | constants.O_NOFOLLOW);
      try {
        const s = fstatSync(fd);
        if (!s.isFile() || s.nlink !== 1 || (s.mode & 0o077) !== 0 || s.uid !== process.getuid?.()) throw new GatedWriteRefusal('ledger_unavailable');
        const bytes = Buffer.from(JSON.stringify(record) + '\n', 'utf8');
        if (writeSync(fd, bytes) !== bytes.length) throw new GatedWriteRefusal('ledger_unavailable');
        fsyncSync(fd);
      } finally { closeSync(fd); }
    } catch (error) {
      throw error instanceof GatedWriteRefusal ? error : new GatedWriteRefusal('ledger_unavailable');
    }
  }

  /** Every record, in the order written. A line that is not a valid record makes the ledger unusable. */
  read(): LedgerRecord[] {
    this.assertAvailable();
    let body: string;
    try { body = readFileSync(this.path, 'utf8'); } catch { throw new GatedWriteRefusal('ledger_unavailable'); }
    const records: LedgerRecord[] = [];
    for (const line of body.split('\n')) {
      if (!line) continue;
      let value: unknown;
      try { value = JSON.parse(line); } catch { throw new GatedWriteRefusal('ledger_unavailable'); }
      if (!validRecord(value)) throw new GatedWriteRefusal('ledger_unavailable');
      records.push(value);
    }
    return records;
  }

  /**
   * R11: take `<ledger>.lock` by an exclusive create, holding `{pid, runId, at}`. A lock whose process is alive
   * (the signal-0 test succeeds or answers EPERM) is another call still running: `attempt_already_open`. A lock
   * of a dead process, or one that cannot be read and is older than a few seconds, is stale: it is removed and
   * `reclaimStale` decides, `false` (a save) leaving the caller to inspect the ledger and refuse
   * `reconcile_blocks_retry` (the holder's `staleRunId` says whose crash it was), `true` (a review) taking the lock.
   */
  acquireLock(runId: string, reclaimStale: boolean, now: () => number = Date.now): LedgerLock | {stale: true; staleRunId: string | null} {
    this.assertAvailable();
    for (let attempt = 0; attempt < 2; attempt++) {
      let fd: number;
      try { fd = openSync(this.lockPath, 'wx', 0o600); }
      catch (error) {
        if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw new GatedWriteRefusal('ledger_unavailable');
        const held = this.#readLock();
        if (held.live) throw new GatedWriteRefusal('attempt_already_open');
        try { unlinkSync(this.lockPath); } catch (e) { if ((e as NodeJS.ErrnoException).code !== 'ENOENT') throw new GatedWriteRefusal('ledger_unavailable'); }
        if (!reclaimStale) return {stale: true, staleRunId: held.runId};
        continue;
      }
      try {
        const bytes = Buffer.from(JSON.stringify({pid: process.pid, runId, at: new Date(now()).toISOString()}) + '\n');
        writeSync(fd, bytes);
        fsyncSync(fd);
      } catch { try { closeSync(fd); } catch { /* the lock is removed below */ } this.#unlinkLock(); throw new GatedWriteRefusal('ledger_unavailable'); }
      closeSync(fd);
      return {staleRunId: undefined, release: () => this.#unlinkLock()};
    }
    throw new GatedWriteRefusal('attempt_already_open');
  }

  #unlinkLock(): void { try { unlinkSync(this.lockPath); } catch { /* already gone */ } }

  #readLock(): {live: boolean; runId: string | null} {
    let parsed: {pid?: unknown; runId?: unknown} | null = null;
    try { parsed = JSON.parse(readFileSync(this.lockPath, 'utf8')) as {pid?: unknown; runId?: unknown}; } catch { parsed = null; }
    const pid = parsed && typeof parsed.pid === 'number' && Number.isSafeInteger(parsed.pid) && parsed.pid > 1 ? parsed.pid : null;
    const runId = parsed && typeof parsed.runId === 'string' && parsed.runId.length <= 80 ? parsed.runId : null;
    if (pid === null) {
      let young = true;
      try { young = Date.now() - statSync(this.lockPath).mtimeMs < LOCK_WRITE_GRACE_MS; } catch { young = false; }
      return {live: young, runId: null};
    }
    return {live: this.pidAlive(pid), runId};
  }
}

/** Whether a process exists: `kill(pid, 0)` succeeds or is refused for permission (EPERM); ESRCH is a dead process. */
export function defaultPidAlive(pid: number): boolean {
  try { process.kill(pid, 0); return true; }
  catch (error) { return (error as NodeJS.ErrnoException).code === 'EPERM'; }
}

/** The ledger path of a launch: `--gated-write-ledger <absPath>`, else `ACADEMY_GATED_WRITE_LEDGER_PATH`; never a default. */
export function gatedWriteLedgerConfiguration(args: readonly string[], env: Record<string, string | undefined> = process.env): {path: string | null; remainingArgs: string[]} {
  const remaining: string[] = [];
  let path: string | null = null;
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--gated-write-ledger') {
      const value = args[i + 1];
      if (value === undefined || value.startsWith('--')) throw new Error('gated_write_ledger_argument_invalid');
      path = value; i++;
    } else remaining.push(args[i]!);
  }
  if (path === null) { const fromEnv = env.ACADEMY_GATED_WRITE_LEDGER_PATH; path = fromEnv !== undefined && fromEnv !== '' ? fromEnv : null; }
  return {path, remainingArgs: remaining};
}
