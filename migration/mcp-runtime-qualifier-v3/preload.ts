// Observer only. Original modules, Date, Date.now and randomUUID stay unchanged.
import fs from 'node:fs';
import fsp from 'node:fs/promises';
import {resolve, isAbsolute, dirname} from 'node:path';
import {syncBuiltinESMExports} from 'node:module';
import {createHash} from 'node:crypto';

const root = process.env.S11_SOURCE_ROOT!, out = process.env.S11_MODE_ROOT!;
const deps = process.env.S11_DEPENDENCY_ROOT!;
if (![root, out, deps].every(p => p && isAbsolute(p) && resolve(p) === p)) throw Error('s11_explicit_canonical_roots_required');
const original = {
  append: fs.appendFileSync.bind(fs), write: fs.writeFileSync.bind(fs),
  mkdir: fs.mkdirSync.bind(fs), read: fs.readFileSync.bind(fs),
  realpath: fs.realpathSync.bind(fs), stat: fs.statSync.bind(fs),
};
if (original.realpath(out) !== out || original.stat(out).uid !== process.getuid?.()) throw Error('s11_observer_root_unqualified');
const dir = resolve(out, 'processes', String(process.pid));
original.mkdir(dir, {recursive: true, mode: 0o700});
const hash = (b: Uint8Array) => createHash('sha256').update(b).digest('hex');
const sample = () => {
  const before = process.hrtime.bigint(), wall = BigInt(Date.now()) * 1000000n, after = process.hrtime.bigint();
  return {wallNs: String(wall), monoNs: String(after), monoBeforeNs: String(before), monoAfterNs: String(after), wallResolutionNs: 1000000};
};
let sequence = 0, eventBytes = 0;
const log = (kind: string, details: object = {}) => {
  const b = Buffer.from(JSON.stringify({pid: process.pid, sequence: ++sequence, kind, sample: sample(), ...details}) + '\n');
  eventBytes += b.length;
  if (eventBytes > 64 * 1024 * 1024) throw Error('s11_observer_event_limit');
  original.append(resolve(dir, 'events.ndjson'), b, {mode: 0o600});
};
const statParts = original.read('/proc/self/stat', 'utf8').slice(original.read('/proc/self/stat', 'utf8').lastIndexOf(')') + 1).trim().split(/\s+/);
const fds: Record<string, string> = {};
for (const fd of [0, 1, 2]) {
  try { fds[String(fd)] = fs.readlinkSync('/proc/self/fd/' + fd); }
  catch { fds[String(fd)] = 'unavailable'; }
}
original.write(resolve(dir, 'START.json'), JSON.stringify({
  pid: process.pid, ppid: process.ppid, pgid: Number(statParts[2]),
  uid: process.getuid?.(), argv: process.argv, execArgv: process.execArgv,
  nativeArgv: original.read('/proc/self/cmdline').toString().split('\0').filter(Boolean),
  fds, cwd: process.cwd(), execPath: process.execPath, sample: sample(),
}) + '\n', {mode: 0o600, flag: 'wx'});

for (const name of ['stdout', 'stderr'] as const) {
  const stream = process[name], write = stream.write.bind(stream);
  let bytes = 0;
  stream.write = ((chunk: any, ...args: any[]) => {
    const b = typeof chunk === 'string' ? Buffer.from(chunk, typeof args[0] === 'string' ? args[0] : undefined) : Buffer.from(chunk);
    bytes += b.length;
    if (bytes > 12 * 1024 * 1024) throw Error('s11_tap_channel_limit');
    // A JS write tap records an attempted write, never an independent OS pipe.
    original.append(resolve(dir, name + '.bin'), b, {mode: 0o600});
    log('stream_write', {channel: name, bytes: b.length, sha256: hash(b), data: b.toString('base64')});
    try { return write(chunk, ...args as []); }
    catch (error) { log('stream_write_failed', {channel: name, error: (error as any)?.code ?? 'Error'}); throw error; }
  }) as any;
}
// No data listener: observe only when the original consumer emits data.
const emit = process.stdin.emit.bind(process.stdin);
let inputBytes = 0;
process.stdin.emit = ((event: any, ...args: any[]) => {
  if (event === 'data') {
    const b = Buffer.from(args[0]); inputBytes += b.length;
    if (inputBytes > 12 * 1024 * 1024) throw Error('s11_tap_channel_limit');
    original.append(resolve(dir, 'stdin.bin'), b, {mode: 0o600});
    log('stdin_data', {bytes: b.length, sha256: hash(b), data: b.toString('base64')});
  }
  return emit(event, ...args);
}) as any;
process.on('exit', code => original.write(resolve(dir, 'EXIT.json'), JSON.stringify({code, at: new Date().toISOString(), sample: sample()}) + '\n', {mode: 0o600}));

const allowed = (p: any) => {
  if (typeof p !== 'string' && !(p instanceof URL) && !Buffer.isBuffer(p)) throw Error('s11_write_path_required');
  const s = resolve(p instanceof URL ? p.pathname : String(p));
  if (s !== out && !s.startsWith(out + '/')) throw Error('s11_write_outside_disposable_root');
  let parent = dirname(s);
  while (!fs.existsSync(parent)) parent = dirname(parent);
  if (original.realpath(parent) !== parent || (fs.existsSync(s) && original.realpath(s) !== s)) throw Error('s11_write_symlink_escape');
  return s;
};
const bytesOf = (v: any, options: any) => typeof v === 'string' ? Buffer.from(v, typeof options === 'string' ? options : options?.encoding) : Buffer.from(v);
const observeFile = (p: string) => p.startsWith(out + '/receipts/') || p === out + '/events.txt' || p === out + '/fault-events.ndjson' || p === out + '/registry.json';
for (const key of ['writeFileSync', 'appendFileSync', 'mkdirSync', 'rmSync', 'unlinkSync', 'chmodSync', 'rmdirSync', 'truncateSync', 'renameSync', 'copyFileSync', 'linkSync', 'symlinkSync'] as const) {
  const fn = (fs as any)[key].bind(fs);
  (fs as any)[key] = (p: any, ...args: any[]) => {
    const path = allowed(p);
    if (['renameSync', 'copyFileSync', 'linkSync', 'symlinkSync'].includes(key)) allowed(args[0]);
    const before = sample();
    try {
      const result = fn(p, ...args);
      if (observeFile(path) && ['writeFileSync', 'appendFileSync'].includes(key)) {
        const b = bytesOf(args[0], args[1]);
        log('file_write', {api: key, path, before, bytes: b.length, sha256: hash(b), data: b.toString('base64'), success: true});
      }
      return result;
    } catch (error) { log('file_operation_failed', {api: key, path, before, error: (error as any)?.code ?? 'Error'}); throw error; }
  };
}
for (const key of ['writeFile', 'appendFile', 'mkdir', 'rm', 'unlink', 'chmod', 'rmdir', 'truncate', 'rename', 'copyFile', 'link', 'symlink'] as const) {
  const fn = (fsp as any)[key].bind(fsp);
  (fsp as any)[key] = (p: any, ...args: any[]) => {
    const path = allowed(p);
    if (['rename', 'copyFile', 'link', 'symlink'].includes(key)) allowed(args[0]);
    // Preserve the original Promise identity and exception. Selected original
    // receipt/event writes are synchronous and observed above. Promise writes
    // get confinement only, never a claimed successful-write observation.
    return fn(p, ...args);
  };
}
syncBuiltinESMExports();
await import(root + '/modules/academy-workbench/tools/academy-mcp/tests/network-deny.ts');
const deny = () => { throw Error('s11_network_forbidden'); };
Bun.connect = deny as any; Bun.listen = deny as any; Bun.serve = deny as any;
const entry = original.read(process.argv[1]!);
log('entry', {path: process.argv[1], sha256: hash(entry), bytes: entry.length});
