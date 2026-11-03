// Fictional upstream only, used to stimulate unchanged neutral-server routing.
// This is not an original protocol oracle. Its replies must never qualify native effects.
import {appendFileSync,existsSync,readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {McpServer} from '@modelcontextprotocol/server';
import {StdioServerTransport} from '@modelcontextprotocol/server/stdio';
import {z} from 'zod';
const base=process.env.S11_MODE_ROOT!,control=resolve(base,'fixture-control.json');
const server=new McpServer({name:'s11-fictional-fault-upstream',version:'1'});
const result=(data:any)=>({content:[{type:'text' as const,text:JSON.stringify(data)}],structuredContent:data});
const name=process.argv[2]??'dailyrecord';
const trace=(v:any)=>appendFileSync(resolve(base,'fault-events.ndjson'),JSON.stringify({pid:process.pid,endpoint:name,...v})+'\n');
const cfg=()=>existsSync(control)?JSON.parse(readFileSync(control,'utf8')):{};
const tools=['academy_source_status','academy_cism_weekly_save','academy_bounded_probe'];
server.registerTool('academy_runtime_read_admission',{inputSchema:z.object({}).strict()},()=>{
 const a=JSON.parse(readFileSync(resolve(base,'admission.json'),'utf8'));
 const admission={sourceNamespace:a.sourceNamespace,teacherId:a.teacherId,allowedFrom:a.allowedFrom,allowedThrough:a.allowedThrough};
 return result({status:'configured',admissionSha256:createHash('sha256').update(JSON.stringify(admission)).digest('hex')});
});
for(const n of tools)server.registerTool(n,{description:'S11 fictional fault stimulus',inputSchema:n==='academy_bounded_probe'?z.object({text:z.string().min(1).max(4),limit:z.number().int().min(1).max(3).default(2)}).strict():n==='academy_cism_weekly_save'?z.object({attribution:z.unknown().optional()}).strict():z.object({}).strict(),annotations:{readOnlyHint:n!=='academy_cism_weekly_save',destructiveHint:false,idempotentHint:false,openWorldHint:false}},async input=>{
 trace({tool:n,input});const c=cfg();
 if(c.delayMs){trace({event:'pending'});await Bun.sleep(c.delayMs);}
 if(c.hang){trace({event:'hung'});await new Promise(()=>{});}
 if(c.error)return {isError:true,...result({status:'blocked',code:c.error,academyWrites:null,sourceRequests:1})};
 if(c.oversize)return result({status:'observed',fictional:'x'.repeat(c.oversize)});
 return result({status:'observed',fictional:true,endpoint:name,receivedArguments:input});
});
await server.connect(new StdioServerTransport());
