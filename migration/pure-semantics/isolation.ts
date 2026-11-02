import http from 'node:http';
import https from 'node:https';
import net from 'node:net';
import tls from 'node:tls';
import crypto from 'node:crypto';
import {syncBuiltinESMExports} from 'node:module';
import {mock} from 'bun:test';

// Invocation-local test instrumentation, never loaded by production.
export const effects = {deniedTransportAttempts: 0, forbiddenFakeDispatches: 0};
function deny(): never { effects.deniedTransportAttempts++; throw Error('s08_transport_denied'); }
globalThis.fetch = Object.assign(deny, {preconnect: deny});
http.request = http.get = https.request = https.get = deny as any;
net.connect = net.createConnection = net.Socket.prototype.connect = tls.connect = deny as any;
// Review digests bind observation handles. Fix their sole random input BEFORE
// importing the unchanged source instead of deleting handles/digests afterward.
let nextUuid = 0;
mock.module('node:crypto', () => ({...crypto, randomUUID: () => `00000000-0000-4000-8000-${String(++nextUuid).padStart(12, '0')}`}));
syncBuiltinESMExports();
export function forbiddenFakeDispatch() { effects.forbiddenFakeDispatches++; }
