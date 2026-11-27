import {expect,test} from 'bun:test';
import {resolve} from 'node:path';
import {Client} from '@modelcontextprotocol/client';
import {StdioClientTransport} from '@modelcontextprotocol/client/stdio';
import {context} from './dailyrecord-fixture.ts';

async function connect(entry:string){
  const client=new Client({name:'synthetic-dailyrecord-verifier',version:'0.1.0'});
  const transport=new StdioClientTransport({command:process.execPath,args:['--preload',resolve(import.meta.dir,'network-deny.ts'),resolve(import.meta.dir,'../src',entry)],env:{PATH:process.env.PATH??'/usr/bin:/bin'},stderr:'pipe'});
  await client.connect(transport);return {client,transport};
}
test('existing label MCP discovers context-required DailyRecord tools while preserving all three label tools',async()=>{
  const {client,transport}=await connect('label-server.ts');
  try{
    const tools=(await client.listTools()).tools;
    expect(tools.map(t=>t.name).sort()).toEqual(['academy_endpoint_identity','academy_student_metadata_get','academy_dailyrecord_execution_status','academy_dailyrecord_get','academy_dailyrecord_intake','academy_dailyrecord_reconcile','academy_dailyrecord_review','academy_dailyrecord_save','academy_find_item_labels','academy_item_answer_lookup','academy_item_section_trace','academy_item_label_scope','academy_review_type_questions','academy_student_resolve','academy_prestudy_get','academy_prestudy_report_get','academy_clinic_paper_status_get','academy_textbook_section_results_get'].sort());
    const saveTool=tools.find(t=>t.name==='academy_dailyrecord_save')!;expect(saveTool.annotations?.readOnlyHint).toBe(false);expect(saveTool.annotations?.destructiveHint).toBe(true);
    const saveRefused=await client.callTool({name:'academy_dailyrecord_save',arguments:{context:context(),observationHandle:'dailyrecord-observation:synthetic',reviewDigest:'9'.repeat(64),proposals:[{field:'progress',value:'- 지정 문제 12문항 풀이 진행.',factRefs:['user-fact:progress'],decision:{kind:'replace',requestRef:context().currentRequestRef}}]}});
    expect(saveRefused.structuredContent).toMatchObject({status:'unavailable',reasonCode:'dailyrecord_save_backend_not_configured',academyWrites:0});
    for(const name of ['academy_dailyrecord_intake','academy_dailyrecord_review']){
      const tool=tools.find(t=>t.name===name)!;expect(tool.annotations?.readOnlyHint).toBe(true);expect(tool.annotations?.destructiveHint).toBe(false);
      // The review needs only proposals (and a handle for a patch). Intake needs a scope, inside context or written at the top level, so it requires no member by name.
      if(name==='academy_dailyrecord_intake')expect(tool.inputSchema.required??[]).toEqual([]);
      else expect(tool.inputSchema.required).toContain('proposals');const refused=await client.callTool({name,arguments:{}});expect(refused.structuredContent).toMatchObject({status:'blocked',code:'tool_input_invalid',academyWrites:0});
    }
    const intake=await client.callTool({name:'academy_dailyrecord_intake',arguments:{context:context()}});
    expect(intake.isError).not.toBe(true);expect(intake.structuredContent).toMatchObject({status:'held',sourceAvailability:'unavailable',academyWrites:0,parentSends:0,observationHandle:null});
    expect((intake.structuredContent as any).userContext.currentRequestRef).toBe(context().currentRequestRef);
    const forged=await client.callTool({name:'academy_dailyrecord_intake',arguments:{context:{...context(),approved:true}}});expect(forged.structuredContent).toMatchObject({status:'blocked',code:'tool_input_invalid',academyWrites:0});
    const review=await client.callTool({name:'academy_dailyrecord_review',arguments:{context:context(),proposals:[{field:'progress',value:'지정 문제 12문항 풀이 진행.',factRefs:['user-fact:progress'],decision:{kind:'replace',requestRef:context().currentRequestRef}}]}});
    expect(review.isError).not.toBe(true);expect(review.structuredContent).toMatchObject({status:'held',observationHandle:null,patch:[],execution:'not_supported'});
  }finally{await client.close();await transport.close();}
},20000);
test('dedicated entrypoint discovers the read-only DailyRecord/metadata/resolver and identity tools with network denied',async()=>{
  const {client,transport}=await connect('dailyrecord-server.ts');
  try{
    const tools=(await client.listTools()).tools;
    expect(tools.map(t=>t.name).sort()).toEqual(['academy_textbook_catalog','academy_textbook_pdf','academy_textbook_toc_get','academy_textbook_units_get','academy_endpoint_identity','academy_student_metadata_get','academy_dailyrecord_execution_status','academy_dailyrecord_get','academy_dailyrecord_intake','academy_dailyrecord_reconcile','academy_dailyrecord_review','academy_dailyrecord_save','academy_item_answer_lookup','academy_item_section_trace','academy_student_resolve','academy_prestudy_get','academy_prestudy_report_get','academy_clinic_paper_status_get','academy_textbook_section_results_get','academy_cism_month','academy_cism_weekly_get','academy_cism_weekly_review','academy_cism_weekly_save'].sort());
    const reportTool=tools.find(t=>t.name==='academy_prestudy_report_get')!;
    expect(reportTool.inputSchema.properties).toHaveProperty('sectionTitles');
    expect(reportTool.inputSchema.properties).toHaveProperty('maxOutputBytes');
    expect(reportTool.inputSchema.required).not.toContain('sectionTitles');
    expect(reportTool.annotations?.readOnlyHint).toBe(true);
    const report=await client.callTool({name:'academy_prestudy_report_get',arguments:{displayName:'Invented Student',prestudyKey:'12345',purpose:'Inspect fictional view',currentRequestRef:'fictional-report-view',sectionTitles:['개념 이해'],maxOutputBytes:4096}});
    expect(report.isError).not.toBe(true);
    expect(report.structuredContent).toMatchObject({status:'unavailable',reasonCode:'prestudy_report_backend_not_configured',academyWrites:0,parentSends:0});
    expect(JSON.parse((report.content as {type:string;text:string}[])[0].text)).toEqual(report.structuredContent);
    const catalog=await client.callTool({name:'academy_textbook_catalog',arguments:{limit:100}});
    expect(catalog.structuredContent).toMatchObject({status:'unavailable',code:'textbook_source_backend_not_configured',academyWrites:0});
    // The batch tool academy_student_metadata is retired; the by-name read stays as the hidden upstream of the DailyTest composition.
    expect(tools.map(t=>t.name)).not.toContain('academy_student_metadata');
    expect(tools.find(t=>t.name==='academy_student_metadata_get')?.annotations?.readOnlyHint).toBe(true);
    const metadata=await client.callTool({name:'academy_student_metadata_get',arguments:{purpose:'Invented read',currentRequestRef:'invented-request',
      displayName:'Invented Student',lessonDate:'2040-01-04',queries:[{kind:'daily_metadata'}]}});
    expect(metadata.structuredContent).toMatchObject({status:'unavailable',reasonCode:'student_metadata_backend_not_configured',academyWrites:0,parentSends:0});
  }
  finally{await client.close();await transport.close();}
},10000);

test('MCP returns an actionable review hold for ordinal questionCounts without an upstream error',async()=>{
  const {client,transport}=await connect('dailyrecord-server.ts');
  try{
    const c=context();c.facts[0].questionCounts=[{count:10,rendering:'유형 10'}];
    const reviewed=await client.callTool({name:'academy_dailyrecord_review',arguments:{context:c,proposals:[{field:'progress',value:'- 유형 10까지 풀이 진행.',factRefs:[c.facts[0].ref],decision:{kind:'set',requestRef:c.currentRequestRef}}]}});
    expect(reviewed.isError).not.toBe(true);
    expect((reviewed.structuredContent as any).rows[0].warnings.map((w:any)=>w.code)).toContain('count_rendering_must_preserve_count');
    const tools=(await client.listTools()).tools;
    expect(tools.find(t=>t.name==='academy_dailyrecord_save')?.description).toContain('request_instruction_conflict');
    expect(tools.find(t=>t.name==='academy_dailyrecord_review')?.description).toContain('parent_internal_language_excluded');
    expect(tools.find(t=>t.name==='academy_dailyrecord_intake')?.description).toContain('5 minutes');
  }finally{await client.close();await transport.close();}
},20000);
