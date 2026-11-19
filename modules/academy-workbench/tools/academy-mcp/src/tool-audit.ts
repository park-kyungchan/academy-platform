import {createHash,randomUUID} from 'node:crypto';
import {mkdirSync,lstatSync,realpathSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import type {McpServer} from '@modelcontextprotocol/server';
import {z} from 'zod';
import {publicReadFailure,publicFailureFromData} from './read-failure.ts';
import {toolResult,withToolMeta} from './tool-result.ts';
const digest=(value:unknown)=>createHash('sha256').update(JSON.stringify(value)??'null').digest('hex');
const statuses=new Set(['observed','resolved','configured','ready_for_review','prepared','reviewed','saved','persisted','held','unavailable','failed','blocked','interview_required','settled','partial','page','refused','available','plan_only','consistent_caller_reported','ready']);
/** Paths contain only declared schema keys and bounded numeric indices, never caller property names. A custom issue also names its refusal `code` when the schema's own message is one (a snake_case code, never caller text). */
export function inputIssues(schema:z.ZodType,input:unknown){
 const parsed=schema.safeParse(input);if(parsed.success)return [];
 const keys=new Set<string>();
 const visit=(value:any)=>{if(!value||typeof value!=='object')return;for(const [k,v] of Object.entries(value)){if(k==='properties'&&v&&typeof v==='object')Object.keys(v).forEach(k=>keys.add(k));visit(v);}};
 visit(z.toJSONSchema(schema,{io:'input',unrepresentable:'any'}));
 return parsed.error.issues.slice(0,16).map(i=>({kind:i.code,...(i.code==='custom'&&/^[a-z][a-z0-9_]{0,95}$/.test(i.message)?{code:i.message}:{}),path:i.path.slice(0,12).map(p=>typeof p==='number'&&p>=0&&p<=100000?p:typeof p==='string'&&keys.has(p)?p:'unknown_key')}));
}
/** Install before registering tools. Public SDK hooks only; no private dispatch-table access.
 * Preflight returns safe input diagnostics before SDK converts validation to opaque text.
 * Domain failures are data results so older neutral bridges preserve their causes.
 */
export function installToolAudit(server:McpServer,options:{root?:string;endpoint?:string;preserveResult?:boolean}={}){
 const schemas=new Map<string,z.ZodType>(),writes=new Set<string>();
 const register=server.registerTool.bind(server);
 server.registerTool=((name:string,config:any,callback:any)=>{
  if(config.inputSchema)schemas.set(name,config.inputSchema);
  if(config.annotations?.readOnlyHint!==true)writes.add(name);
  if(options.preserveResult)return (register as any)(name,config,callback);
  return (register as any)(name,config,async(...args:any[])=>{
   try{return await callback(...args);}catch(error){
    const failure=publicReadFailure(error,'tool_handler_failed'),possibleWrite=writes.has(name);
    return toolResult({status:'blocked',code:failure.code,failure,academyWrites:possibleWrite?null:0,...(possibleWrite?{effectStatus:'unknown'}:{}),parentSends:0,automaticRetryAllowed:false});
   }
  });
 }) as typeof server.registerTool;
 const protocol=server.server,set=protocol.setRequestHandler.bind(protocol);
 protocol.setRequestHandler=((method:string,handler:any)=>{
  if(method!=='tools/call')return (set as any)(method,handler);
  return (set as any)(method,async(request:any,ctx:any)=>{
   const name=schemas.has(request.params.name)?request.params.name:'unregistered_tool',occurrence=randomUUID(),started=Date.now();
   const base={schemaVersion:'academy.tool-audit/1',occurrence,tool:name,endpoint:options.endpoint??'local',inputSha256:digest(request.params.arguments??{}),automaticRetryAllowed:false};
   const persist=(phase:string,data:unknown)=>{
    if(!options.root){if(options.preserveResult)process.stderr.write(JSON.stringify({...base,phase,...data as object})+'\n');return;}
    mkdirSync(options.root,{recursive:true,mode:0o700});const st=lstatSync(options.root);
    if(!st.isDirectory()||st.isSymbolicLink()||realpathSync(options.root)!==resolve(options.root)||(st.mode&0o077)!==0||st.uid!==process.getuid?.())throw Error('tool_audit_store_unqualified');
    writeFileSync(resolve(options.root,occurrence+'-'+phase+'.json'),JSON.stringify({...base,phase,...data as object})+'\n',{mode:0o600,flag:'wx'});
   };
   const schema=schemas.get(name);
   let invoked=false,result:any,phase='input_validation';
   try{
    persist('accepted',{observedAt:new Date().toISOString()});
    const issues=schema&&typeof schema.safeParse==='function'?inputIssues(schema,request.params.arguments??{}):[];
    if(!schema)result=toolResult({status:'blocked',code:'tool_not_registered',failure:{category:'endpoint_absence',code:'tool_not_registered',evidenceLocator:'src/tool-audit.ts:dispatch'},academyWrites:0,parentSends:0,automaticRetryAllowed:false});
    else if(issues.length)result=toolResult({status:'blocked',code:'tool_input_invalid',failure:{category:'input_validation',code:'tool_input_invalid',evidenceLocator:'src/tool-audit.ts:inputIssues'},issues,academyWrites:0,parentSends:0,automaticRetryAllowed:false});
    else{
     phase='handler';invoked=true;result=await handler(request,ctx);
     if(result?.isError&&!options.preserveResult){
      // Only exact finite public codes survive. Never copy SDK/upstream text or payloads.
      const text=result.content?.length===1&&result.content[0]?.type==='text'?result.content[0].text:'';
      const failure=publicReadFailure(Error(text),'tool_handler_failed');
      result=toolResult({status:'blocked',code:failure.code,failure,academyWrites:writes.has(name)?null:0,...(writes.has(name)?{effectStatus:'unknown'}:{}),parentSends:0,automaticRetryAllowed:false});
     }
    }
    const data=result.structuredContent??{},failure=data.failure?publicFailureFromData(data.failure):typeof (data.code??data.reasonCode)==='string'?publicFailureFromData({code:data.code??data.reasonCode}):null,audit={...base,phase,failure,elapsedMs:Date.now()-started,status:statuses.has(data.status)?data.status:null,resultSha256:digest(result),effectStatus:data.effectStatus??(data.academyWrites===0?'none':null)};
    persist('completed',audit);if(options.preserveResult)return result;
    // Oversize metadata refusals have a tighter established wire bound; the
    // private receipt still retains the audit if public metadata cannot fit.
    const cap=data.reasonCode==='student_metadata_unqualified_no_automatic_retry'?1023:512*1024;
    return withToolMeta(result,{'academy/audit':audit},cap);
   }catch(error){
    const failure=publicReadFailure(error,'tool_handler_failed'),possibleWrite=invoked&&writes.has(name);
    const data={status:'blocked',code:failure.code,failure,occurrence,phase,academyWrites:possibleWrite?null:0,...(possibleWrite?{effectStatus:'unknown'}:{}),parentSends:0,automaticRetryAllowed:false};
    try{persist('failed',{phase,failure,elapsedMs:Date.now()-started,academyWrites:data.academyWrites});}catch{/* Receipt failure cannot deny a possible write or trigger replay. */}
    return toolResult(data);
   }
  });
 }) as typeof protocol.setRequestHandler;
}
