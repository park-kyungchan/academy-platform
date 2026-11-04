import http from 'node:http';
import https from 'node:https';
import net from 'node:net';
import tls from 'node:tls';
import crypto from 'node:crypto';
import {syncBuiltinESMExports} from 'node:module';
import {mock} from 'bun:test';
export const effects = {deniedTransportAttempts:0,forbiddenFakeDispatches:0};
const deny = ():never => {effects.deniedTransportAttempts++;throw Error('extension_transport_denied');};
globalThis.fetch = Object.assign(deny,{preconnect:deny});
http.request=http.get=https.request=https.get=deny as any;
net.connect=net.createConnection=net.Socket.prototype.connect=tls.connect=deny as any;
let next=0;
export const uuidEvents:string[]=[];
// Test port installed before ANY business module import or intake/digest call.
mock.module('node:crypto',()=>({...crypto,randomUUID:()=>{
 const value=`00000000-0000-4000-8000-${String(++next).padStart(12,'0')}`;
 uuidEvents.push(value);return value;
}}));
syncBuiltinESMExports();
