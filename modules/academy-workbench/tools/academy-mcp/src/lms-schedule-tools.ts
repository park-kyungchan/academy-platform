import {z} from 'zod';
import type {McpServer} from '@modelcontextprotocol/server';
import {readFailureResult} from './read-failure.ts';
import {academyRoutingDescription} from './routing-context.ts';
import {LMS_SCHEDULE_VIEWS,lmsScheduleInputIssues,type LmsScheduleInput,type LmsScheduleRead} from '../../../../../apps/academy/dc-gang-a/src/lms/lmsScheduleRead';

/** `academy_lms_schedule_get`: the three read-only LMS pages that had no tool (specs/academy-mcp/lms-reads.md): the
 * teacher's attendance (`AttendanceServlet` Main), the study schedule and the course study manager
 * (`CourseManageServlet` `StudySchedule`, `CourseStudyManager`). One GET of one page per call, under the shared
 * account lock that the endpoint launch takes. Input is qualified here, before any LMS request: each `view` takes
 * only its own members (R1), a date is a real `YYYY-MM-DD` (R7), a period is at most 31 days and, for attendance,
 * within one month (one GET of the servlet answers one month, so a longer period would silently omit rows). */
const text=z.string();
export const lmsScheduleGetSchema=z.object({
  view:z.enum(LMS_SCHEDULE_VIEWS),
  date:text.optional(),
  period:z.object({from:text,through:text}).strict().optional(),
  groupKey:text.optional(),studentKey:text.optional(),gradeNo:text.optional(),semesterNo:text.optional(),levelNo:text.optional(),courseKey:text.optional(),
  status:text.optional(),
}).strict().superRefine((value,context)=>{
  for(const issue of lmsScheduleInputIssues(value as Record<string,unknown>))context.addIssue({code:'custom',path:issue.path,message:issue.message});
});
export type LmsScheduleGet=z.infer<typeof lmsScheduleGetSchema>;
export type LmsScheduleObservation=LmsScheduleRead&{observedAt:string;coverage:string};

export interface LmsScheduleBackend {
  /** One page of the view (a GET, a read of the LMS). */
  readLmsSchedule(input:LmsScheduleInput):Promise<LmsScheduleObservation>;
}

const MAX_RESULT_BYTES=512*1024;
export class LmsScheduleService {
  #tail:Promise<unknown>=Promise.resolve();
  constructor(readonly backend:LmsScheduleBackend|null){}
  /** One LMS session serves one call at a time. */
  #serial<T>(run:()=>Promise<T>):Promise<T>{
    const next=this.#tail.then(run,run);
    this.#tail=next.catch(()=>undefined);
    return next;
  }
  async get(value:unknown){
    // A refused input throws here, before any request.
    const input=lmsScheduleGetSchema.parse(value);
    const base={view:input.view,academyWrites:0,parentSends:0,automaticRetryAllowed:false,
      execution:'read_only' as const,sourceContentIsUntrustedData:true};
    if(!this.backend)return {...base,status:'unavailable' as const,reasonCode:'lms_schedule_backend_not_configured'};
    try{
      const observation=await this.#serial(()=>this.backend!.readLmsSchedule(input as LmsScheduleInput));
      if(observation.view!==input.view)throw Error('scope_conflict');
      const result={...base,status:'observed' as const,...observation};
      if(Buffer.byteLength(JSON.stringify(result),'utf8')>MAX_RESULT_BYTES)throw Error('lms_schedule_result_too_large');
      return result;
    }catch(error){return {...base,status:'unavailable' as const,...readFailureResult(error,'lms_schedule_unqualified_no_automatic_retry')};}
  }
}

/** academy_lms_schedule_get on the dailyrecord endpoint (src/dailyrecord-server.ts). */
export function registerLmsScheduleTool(server:McpServer,service:LmsScheduleService){
  const result=(value:unknown)=>{const structuredContent=JSON.parse(JSON.stringify(value)) as Record<string,unknown>;
    return {content:[{type:'text' as const,text:JSON.stringify(structuredContent)}],structuredContent};};
  server.registerTool('academy_lms_schedule_get',{
    title:'Read the attendance, study schedule or course study manager page',
    description:academyRoutingDescription('academy_lms_schedule_get','view attendance: the lesson rows of one month (lessonDate, courseName, lessonType, attendanceStatus, memo), filtered to date or period, with the group and student filters; study_schedule: the study schedule rows with the grade, group, student and course filters; course_study_manager: the course study manager rows with the grade, semester, level and status filters. Each view takes only its own members; date and period are YYYY-MM-DD, a period is at most 31 days and, for attendance, within one month. One GET of one page; no write, mark, schedule change or automatic retry.'),
    inputSchema:lmsScheduleGetSchema,annotations:{readOnlyHint:true,destructiveHint:false,idempotentHint:false,openWorldHint:service.backend!==null},
  },async input=>result(await service.get(input)));
}
