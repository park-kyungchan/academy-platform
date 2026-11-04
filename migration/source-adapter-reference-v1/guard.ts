import {appendFileSync} from 'node:fs';
import net from 'node:net';
import tls from 'node:tls';
import http from 'node:http';
import https from 'node:https';
import dgram from 'node:dgram';
import child from 'node:child_process';
let traceFile:string|undefined;
export function configureTrace(path:string){traceFile=path;}
export function trace(event:unknown){if(traceFile)appendFileSync(traceFile,JSON.stringify(event)+'\n');}
export const effects={liveAttempts:0,socketAttempts:0,childAttempts:0,fakeDispatches:0,deniedOperations:0,mutatingFakeDispatches:0,sourceHandlersExecuted:false};
const deny=()=>{effects.liveAttempts++;trace({event:'live_attempt_denied'});throw Error('s09_live_transport_denied');};
globalThis.fetch=async()=>deny();
const socket=()=>{effects.socketAttempts++;trace({event:'socket_attempt_denied'});throw Error('s09_socket_denied');};
net.Socket.prototype.connect=socket as any;net.connect=socket as any;net.createConnection=socket as any;tls.connect=socket as any;
http.request=socket as any;http.get=socket as any;https.request=socket as any;https.get=socket as any;dgram.createSocket=socket as any;
const denyChild=()=>{effects.childAttempts++;trace({event:'child_attempt_denied'});throw Error('s09_unselected_child_denied');};
for(const key of ['spawn','spawnSync','exec','execSync','execFile','execFileSync','fork']) (child as any)[key]=denyChild;
Bun.connect=socket as any;Bun.listen=socket as any;Bun.serve=socket as any;Bun.spawn=denyChild as any;Bun.spawnSync=denyChild as any;
export function resetEffects(){for(const k of Object.keys(effects))if(typeof (effects as any)[k]==='number')(effects as any)[k]=0;}
