import http from 'node:http';
import https from 'node:https';
import net from 'node:net';
import tls from 'node:tls';
import {syncBuiltinESMExports} from 'node:module';

// Local test isolation, not a claim about AcademyHttpClient authorization.
export const effects = {deniedTransportAttempts: 0, forbiddenFakeDispatches: 0};
function deny(): never {
  effects.deniedTransportAttempts++;
  throw Error('s05_transport_denied');
}
globalThis.fetch = Object.assign(deny, {preconnect: deny});
http.request = deny as typeof http.request;
http.get = deny as typeof http.get;
https.request = deny as typeof https.request;
https.get = deny as typeof https.get;
net.connect = deny as typeof net.connect;
net.createConnection = deny as typeof net.createConnection;
net.Socket.prototype.connect = deny as typeof net.Socket.prototype.connect;
tls.connect = deny as typeof tls.connect;
syncBuiltinESMExports();

// A deliberately effectful local fake for judge qualification. Never network.
export function forbiddenFakeDispatch(): void { effects.forbiddenFakeDispatches++; }
