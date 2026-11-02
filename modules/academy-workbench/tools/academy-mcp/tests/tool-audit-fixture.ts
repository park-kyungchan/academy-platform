import './network-deny.ts';
import {McpServer} from '@modelcontextprotocol/server';
import {StdioServerTransport} from '@modelcontextprotocol/server/stdio';
import {z} from 'zod';
import {appendFileSync} from 'node:fs';
import {installToolAudit} from '../src/tool-audit.ts';
import {AcademyReadFailure} from '../src/read-failure.ts';
const server=new McpServer({name:'fictional-tool-audit',version:'1'});
installToolAudit(server,{root:process.argv[2],endpoint:'synthetic'});
server.registerTool('fictional_read',{inputSchema:z.object({value:z.number().int().min(1),mode:z.enum(['valid','known','private','auth']).default('valid')}).strict(),annotations:{readOnlyHint:true}},async input=>{
 appendFileSync(process.argv[3],'read\n');
 if(input.mode==='known')throw Error('prestudy_search_form_drift');
 if(input.mode==='private')throw Error('PRIVATE_EXCEPTION_CANARY');
 if(input.mode==='auth')throw new AcademyReadFailure('authentication','source_authentication_failed','ignored',Object.assign(Error('source_http_failed'),{httpStatus:503,sourceOperation:'login_document'}));
 return {structuredContent:{status:'observed'},content:[{type:'text',text:'{"status":"observed"}'}]};
});
server.registerTool('fictional_write',{inputSchema:z.object({}).strict(),annotations:{readOnlyHint:false}},async()=>{appendFileSync(process.argv[3],'effect\n');throw Error('PRIVATE_WRITE_CANARY');});
await server.connect(new StdioServerTransport());
