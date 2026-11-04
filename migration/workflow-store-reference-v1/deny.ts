// Preloaded before any original import. The oracle has no transport authority.
import fs from 'node:fs';
import http from 'node:http';
import https from 'node:https';
import net from 'node:net';
import tls from 'node:tls';
import dgram from 'node:dgram';
import {syncBuiltinESMExports} from 'node:module';
const deny=()=>{const p=process.env.S10_DENIED_LOG;if(p)fs.appendFileSync(p,'denied_transport\n');throw Error('s10_live_transport_denied');};
globalThis.fetch=deny as never;
globalThis.WebSocket=class {constructor(){deny();}} as never;
http.request=deny as never;http.get=deny as never;https.request=deny as never;https.get=deny as never;
net.connect=deny as never;net.createConnection=deny as never;net.Socket.prototype.connect=deny as never;
tls.connect=deny as never;dgram.createSocket=deny as never;
Bun.connect=deny as never;Bun.listen=deny as never;Bun.serve=deny as never;
syncBuiltinESMExports();
