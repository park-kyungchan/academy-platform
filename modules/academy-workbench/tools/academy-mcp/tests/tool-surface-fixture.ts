import './network-deny.ts';
import {appendFileSync} from 'node:fs';
import {McpServer} from '@modelcontextprotocol/server';
import {StdioServerTransport} from '@modelcontextprotocol/server/stdio';
import {z} from 'zod';
import {dailyRecordWorkflowToolSchemas,prestudyGetSchema,prestudyReportGetSchema,textbookSectionResultsGetSchema,clinicPaperStatusGetSchema,itemSectionTraceSchema} from '../src/dailyrecord.ts';
import {itemAnswerLookupSchema} from '../src/item-answer-lookup.ts';
import {studentResolveInputSchema} from '../src/student-resolver.ts';
import {cismGetInput,cismMonthInput,cismReviewInput,cismSaveInput} from '../src/cism-weekly-tools.ts';
import {textbookCatalogInput,textbookPdfInput} from '../src/textbook-source-tools.ts';
import {textbookTocInput} from '../src/textbook-toc.ts';
import {officialReportPdfGetSchema} from '../src/official-report-pdf.ts';
import {paperPrintPdfGetSchema} from '../src/paper-print-pdf.ts';
import {lmsScheduleGetSchema} from '../src/lms-schedule-tools.ts';
import {clinicPaperReviewSchema,clinicPaperSaveSchema} from '../src/clinic-write-tools.ts';

// A recorded synthetic endpoint for tests/tool-surface.test.ts: it registers, with the
// endpoint programs' own input schemas where they export them, every endpoint tool that
// the tools of release/surface.json route to, and answers each call with invented
// values. Each call is recorded as one line `<endpoint> <tool> <arguments JSON>`.
// Usage: bun tool-surface-fixture.ts <endpoint> <events file>
const [endpoint,events]=process.argv.slice(2) as [string,string];
const server=new McpServer({name:'fictional-'+endpoint,version:'1.0.0'});
const key=z.string().regex(/^[1-9][0-9]{0,15}$/),code=z.string().regex(/^#[23456789ABCDEFGHJKLMNPQRSTUVWXYZ]{1,11}$/);
const selectors=z.array(z.enum(['11','12','13','14','15','16']));
const answers:Record<string,(input:any)=>Record<string,unknown>>={
 academy_student_resolve:input=>input.displayName==='Invented Student'?{status:'resolved',studentKey:'77001',displayName:input.displayName,teacherId:'99001'}
  :{status:'unavailable',reasonCode:'student_name_not_found'},
 academy_prestudy_get:input=>({status:'observed',displayName:input.displayName,student:{studentId:'77001',displayName:input.displayName},
  // specs/academy-mcp/lms-reads.md R1: a view answers the lists of its own requests only.
  ...(input.view==='teacher_summary'?{}:{waiting:[{unitLabel:'Invented unit 1',dateLiteral:'2040-01-15',statusLiteral:'invented waiting',prestudyKey:'5001'}]}),
  ...(input.view==='videos'||input.view===undefined?{completed:[{unitLabel:'Invented unit 2',dateLiteral:'2040-01-14',statusLiteral:'invented done',prestudyKey:'5002'}]}:{}),
  ...(input.view==='incomplete'?{}:{summary:[{unitLabel:'Invented unit 2',dateLiteral:'2040-01-14',aiScoreLiteral:'80',ratingLiteral:'3',reportAvailable:true,videoAvailable:true}]}),teacherJudgment:null}),
 academy_lms_schedule_get:input=>({view:input.view,rows:[],observedAt:'2040-01-16T09:00:00.000Z',coverage:'invented_fixture_page'}),
};
const tools:Record<string,Record<string,z.ZodType>>={
 dailyrecord:{
  ...dailyRecordWorkflowToolSchemas,
  academy_student_resolve:studentResolveInputSchema,
  academy_student_metadata_get:z.object({}).passthrough(),
  academy_textbook_section_results_get:textbookSectionResultsGetSchema,academy_clinic_paper_status_get:clinicPaperStatusGetSchema,
  academy_item_section_trace:itemSectionTraceSchema,academy_item_answer_lookup:itemAnswerLookupSchema,
  academy_textbook_catalog:textbookCatalogInput,academy_textbook_toc_get:textbookTocInput,academy_textbook_pdf:textbookPdfInput,
  academy_prestudy_get:prestudyGetSchema,academy_prestudy_report_get:prestudyReportGetSchema,
  academy_cism_weekly_get:cismGetInput,academy_cism_month:cismMonthInput,academy_cism_weekly_review:cismReviewInput,academy_cism_weekly_save:cismSaveInput,
  academy_official_report_pdf_get:officialReportPdfGetSchema,academy_paper_print_pdf_get:paperPrintPdfGetSchema,
  academy_lms_schedule_get:lmsScheduleGetSchema,
  academy_clinic_paper_review:clinicPaperReviewSchema,academy_clinic_paper_save:clinicPaperSaveSchema,
 },
 paper1:{
  academy_daily_test_inventory:z.object({studentId:key,periods:selectors.optional()}).strict(),
  academy_paper_headers:z.object({studentId:key,paperCodes:z.array(code).min(1).max(32)}).strict(),
  academy_paper_definition_lookup:z.object({studentId:key,paperCode:code,view:z.enum(['definition','header']).default('definition')}).strict(),
  academy_printed_paper_lookup:z.object({studentId:key,paperCode:code,period:z.object({from:z.iso.date(),through:z.iso.date()}).strict()}).strict(),
  academy_paper_read_contracts:z.object({studentId:key}).strict(),
  academy_source_read_contracts:z.object({studentId:key,kind:z.enum(['printed_code','daily_test_catalog','middle1_definitions','elementary5_fraction_definitions']),
   unitCodes:z.array(z.string()).optional(),unitNames:z.record(z.string(),z.string()).optional()}).strict(),
 },
 saved:{
  academy_capabilities:z.object({view:z.enum(['hierarchy','operations','source_contract','workflow_rules']).default('hierarchy'),workflow:z.string().optional(),
   domain:z.string().optional(),operationId:z.string().optional(),offset:z.number().int().min(0).default(0),limit:z.number().int().min(1).max(100).default(10)}).strict(),
 },
};
for(const [name,inputSchema] of Object.entries(tools[endpoint]??{})){
 server.registerTool(name,{inputSchema:inputSchema as z.ZodObject},(input:any)=>{
  // The bridge's observed caller attribution varies per call; it is not part of the request.
  const {attribution:_attribution,...request}=input;
  appendFileSync(events,`${endpoint} ${name} ${JSON.stringify(request)}\n`);
  const data={tool:name,endpoint,received:request,status:'observed',...answers[name]?.(request)};
  return {content:[{type:'text',text:JSON.stringify(data)}],structuredContent:data};
 });
}
await server.connect(new StdioServerTransport());
