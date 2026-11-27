import {expect,test} from 'bun:test';
import {mkdtemp,chmod,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createServer,connect,createConnection,Socket} from 'node:net';
import './network-deny.ts';

test('the guard lets a Unix socket path connect and still refuses a TCP connect',async()=>{
 const directory=await mkdtemp(join(tmpdir(),'network-deny-'));await chmod(directory,0o700);
 const path=join(directory,'echo.sock');
 const server=createServer(client=>{client.on('error',()=>{});client.end('pong');});
 try{
  await new Promise<void>((resolve,reject)=>{server.once('error',reject);server.listen(path,resolve);});
  for(const open of [()=>new Socket().connect(path),()=>connect(path),()=>createConnection(path),()=>connect({path})]){
   const reply=await new Promise<string>((resolve,reject)=>{
    const client=open();let text='';client.on('data',chunk=>{text+=chunk.toString();});client.on('end',()=>resolve(text));client.on('error',reject);});
   expect(reply).toBe('pong');
  }
  const blocked=[()=>connect(1,'127.0.0.1'),()=>connect(1),()=>connect('1','127.0.0.1'),()=>connect({port:1,host:'127.0.0.1'}),
   ()=>connect({port:1,path}),()=>createConnection(1,'127.0.0.1'),()=>createConnection({port:1,host:'127.0.0.1'}),()=>connect({host:'127.0.0.1'}),
   ()=>new Socket().connect(1,'127.0.0.1')];
  for(const open of blocked)expect(open).toThrow('verification_outbound_network_forbidden');
 }finally{server.close();await rm(directory,{recursive:true,force:true});}
});
