import {spawn,type ChildProcess} from 'node:child_process';
import {ReadBuffer,serializeMessage,type JSONRPCMessage,type Transport} from '@modelcontextprotocol/client';
import {getDefaultEnvironment,type StdioServerParameters} from '@modelcontextprotocol/client/stdio';

// One per-call endpoint, started in its own process group, so that closing
// the call ends every process the endpoint command started.
//
// The installed endpoint command is `flock --wait 45 <account lock> python3
// native-endpoint.py ...`, which execs setpriv and then bun. flock(1) forks
// the command, the child keeps the locked descriptor, and flock does not
// forward SIGTERM. The SDK's StdioClientTransport.close() signals only its
// direct child (flock): after a timeout the bun endpoint was left running its
// request with the account lock held, and the next call's own `flock --wait
// 45` expired. Signalling the group reaches that child. The account lock
// itself is unchanged: the next endpoint still takes it before any LMS use.

const delay=(ms:number)=>new Promise<void>(resolve=>setTimeout(resolve,ms).unref?.());
const groupAlive=(pgid:number)=>{
  try{process.kill(-pgid,0);return true;}
  catch(error){return (error as NodeJS.ErrnoException).code==='EPERM';}
};
const signalGroup=(pgid:number,signal:NodeJS.Signals)=>{try{process.kill(-pgid,signal);}catch{}};
async function groupGone(pgid:number,ms:number){
  const until=Date.now()+ms;
  while(groupAlive(pgid)){if(Date.now()>=until)return false;await delay(20);}
  return true;
}
// Groups of calls in flight; a bridge that exits ends them as well.
const liveGroups=new Set<number>();
process.once('exit',()=>{for(const pgid of liveGroups)signalGroup(pgid,'SIGKILL');});

export class ProcessGroupStdioTransport implements Transport {
  onclose?:()=>void;
  onerror?:(error:Error)=>void;
  onmessage?:(message:JSONRPCMessage)=>void;
  readonly #params:StdioServerParameters;
  readonly #buffer=new ReadBuffer();
  readonly #graceMs:number;
  #process:ChildProcess|undefined;
  #pgid:number|undefined;
  #closing:Promise<void>|undefined;
  /** graceMs: how long the endpoint may take to exit by itself after stdin ends, and after SIGTERM. */
  constructor(params:StdioServerParameters,graceMs=2000){this.#params=params;this.#graceMs=graceMs;}
  get pid(){return this.#pgid??null;}
  async start(){
    if(this.#process)throw Error('upstream_transport_already_started');
    await new Promise<void>((resolve,reject)=>{
      const child=spawn(this.#params.command,this.#params.args??[],{env:{...getDefaultEnvironment(),...this.#params.env},
        stdio:['pipe','pipe','pipe'],shell:false,cwd:this.#params.cwd,detached:true});
      this.#process=child;
      child.once('error',error=>{reject(error);this.onerror?.(error);});
      child.once('spawn',()=>{this.#pgid=child.pid;if(child.pid)liveGroups.add(child.pid);resolve();});
      child.once('close',()=>{if(!this.#closing)void this.close();});
      child.stdin?.on('error',error=>this.onerror?.(error));
      child.stdout?.on('error',error=>this.onerror?.(error));
      // Endpoint diagnostics are not forwarded; reading them keeps a full pipe from blocking the endpoint.
      child.stderr?.resume();
      child.stdout?.on('data',(chunk:Buffer)=>{
        this.#buffer.append(chunk);
        while(true){
          let message:JSONRPCMessage|null;
          try{message=this.#buffer.readMessage();}catch(error){this.onerror?.(error as Error);continue;}
          if(message===null)break;
          this.onmessage?.(message);
        }
      });
    });
  }
  async send(message:JSONRPCMessage){
    const stdin=this.#process?.stdin;
    if(!stdin||this.#closing)throw Error('upstream_transport_not_connected');
    if(!stdin.write(serializeMessage(message)))await new Promise<void>(resolve=>stdin.once('drain',()=>resolve()));
  }
  close(){
    this.#closing??=this.#teardown();
    return this.#closing;
  }
  async #teardown(){
    const child=this.#process,pgid=this.#pgid;
    if(child&&child.exitCode===null&&child.signalCode===null){
      // As before: a finished endpoint exits by itself once its stdin ends.
      const exited=new Promise<void>(resolve=>child.once('exit',()=>resolve()));
      try{child.stdin?.end();}catch{}
      await Promise.race([exited,delay(this.#graceMs)]);
    }
    if(pgid!==undefined){
      if(groupAlive(pgid)){
        signalGroup(pgid,'SIGTERM');
        if(!await groupGone(pgid,this.#graceMs)){signalGroup(pgid,'SIGKILL');await groupGone(pgid,this.#graceMs);}
      }
      liveGroups.delete(pgid);
    }
    for(const stream of [child?.stdin,child?.stdout,child?.stderr])try{stream?.destroy();}catch{}
    this.#buffer.clear();
    this.#process=undefined;
    this.onclose?.();
  }
}
