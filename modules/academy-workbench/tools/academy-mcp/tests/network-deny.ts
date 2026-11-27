import http from 'node:http';
import https from 'node:https';
import net from 'node:net';
import tls from 'node:tls';
import { syncBuiltinESMExports } from 'node:module';

function deny(): never {
  process.stderr.write('verification_outbound_network_attempt_blocked\n');
  throw Error('verification_outbound_network_forbidden');
}
globalThis.fetch = Object.assign(deny, { preconnect: deny });
http.request = deny as typeof http.request;
http.get = deny as typeof http.get;
https.request = deny as typeof https.request;
https.get = deny as typeof https.get;
// A connection to a Unix socket path stays local and is allowed; a port or a host is denied.
function isUnixPath(args: unknown[]): boolean {
  let first = args[0];
  if (Array.isArray(first)) first = first[0];
  if (typeof first === 'string') return Number.isNaN(Number(first)) || first.trim() === '';
  if (typeof first === 'object' && first !== null) {
    const options = first as {path?: unknown; port?: unknown};
    return typeof options.path === 'string' && options.port === undefined;
  }
  return false;
}
function guardUnixOnly<F extends (...args: any[]) => any>(original: F): F {
  return function guarded(this: unknown, ...args: unknown[]) {
    if (!isUnixPath(args)) return deny();
    return original.apply(this, args);
  } as unknown as F;
}
net.connect = guardUnixOnly(net.connect);
net.createConnection = guardUnixOnly(net.createConnection);
net.Socket.prototype.connect = guardUnixOnly(net.Socket.prototype.connect);
tls.connect = deny as typeof tls.connect;
syncBuiltinESMExports();
