import './network-deny.ts';
import {appendFileSync} from 'node:fs';
import {McpServer} from '@modelcontextprotocol/server';
import {StdioServerTransport} from '@modelcontextprotocol/server/stdio';
import {z} from 'zod';
// A fictional endpoint started as `flock <lock> bun <this file>`, like the
// installed native endpoints: the trace hangs past the bridge's bound and the
// process stays alive after stdin closes, holding the inherited lock.
const events=process.argv[2]!;
appendFileSync(events,'start '+process.pid+'\n');
const server=new McpServer({name:'fictional-hanging-upstream',version:'1.0.0'});
const input=z.object({label:z.string().regex(/^[0-9]{1,8}$/)}).strict();
server.registerTool('academy_item_section_trace',{inputSchema:input},async()=>{
  appendFileSync(events,'trace-started\n');
  await new Promise(resolve=>setTimeout(resolve,120000));
  appendFileSync(events,'trace-ended\n');
  return {content:[{type:'text',text:'{}'}],structuredContent:{status:'observed'}};
});
server.registerTool('academy_item_answer_lookup',{inputSchema:input},async({label})=>{
  appendFileSync(events,'lookup\n');
  const data={status:'observed',label};
  return {content:[{type:'text',text:JSON.stringify(data)}],structuredContent:data};
});
await server.connect(new StdioServerTransport());
