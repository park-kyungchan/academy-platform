import {z} from 'zod';
import {dailyRecordContextSchema,dailyRecordFields,dailyRecordIdSchema} from './dailyrecord-context.ts';
import {academyDateInputSchema,requestIdentityShape,resolveAcademyDate,withRequestIdentity} from './read-input.ts';
export {academyBusinessDate} from './read-input.ts';
import {publicFailureFromData} from './read-failure.ts';
import {UpstreamToolFailure} from './upstream-failure.ts';

export const dailyRecordGetSchema=z.object({
  displayName:z.string().trim().min(1).max(200).describe('Exact student display name from the current user request; never guess a native key.'),
  ...requestIdentityShape,
  lessonDate:academyDateInputSchema.default('today').describe('Omit for today. '+academyDateInputSchema.description),
  view:z.enum(['full','compact']).optional(),
}).strict();

/** The configured teacher of the owner backend. Never part of tool input. */
export type DailyRecordTeacherScope={sourceNamespace:'academy.lms.dayrecord';teacherId:string};
export interface DailyRecordReadRoutePort {
  resolveStudent(input:unknown):Promise<unknown>;
  intake(input:unknown):Promise<unknown>;
}

export const dailyRecordGetDescription='Read live DailyRecord by exact student name. Preferred for requests such as “오늘 학생 DailyRecord 조회해봐” or current/today DailyRecord lookups. Omit lessonDate for today in Asia/Seoul. The owner backend supplies namespace and teacher; this tool resolves the name to a native key on the lesson date then calls academy_dailyrecord_intake. Never use saved academy_query or academy_source_status to decide live availability. Ambiguous or unknown names or failed source reads stop with a named blocker; no guesses or automatic retry. Returns observed fields even when authoring would require a memo/assessment interview. No authoring, save or send.';


const resolutionSchema=z.discriminatedUnion('status',[
  // teacherId: the resolving backend's configured teacher, which its intake also requires.
  z.object({status:z.literal('resolved'),studentKey:z.string().regex(/^[1-9][0-9]{0,15}$/),displayName:z.string(),teacherId:z.string().regex(/^[1-9][0-9]{0,15}$/).optional()}),
  z.object({status:z.literal('unavailable'),reasonCode:dailyRecordIdSchema,failure:z.unknown().optional(),upstreamOccurrence:z.uuid().optional()}),
]);
const hash=z.string().regex(/^[a-f0-9]{64}$/);
const targetSchema=dailyRecordContextSchema.shape.scope.extend({courseId:dailyRecordIdSchema,membershipId:dailyRecordIdSchema,recordId:dailyRecordIdSchema});
const intakeReadSchema=z.object({
  userContext:dailyRecordContextSchema,status:z.enum(['held','interview_required','ready_for_review']),
  sourceAvailability:z.enum(['observed','unavailable']),blockers:z.array(dailyRecordIdSchema),
  source:z.object({observedAt:z.iso.datetime({offset:true}),sourceRevision:hash,wireDigest:hash,transmissionState:z.enum(['unknown','not_sent','already_sent'])}).nullable(),
  fields:z.array(z.object({field:z.enum(dailyRecordFields),selected:z.boolean(),disposition:z.string(),target:targetSchema.nullable(),
    before:z.discriminatedUnion('state',[
      z.object({state:z.literal('known'),value:z.string(),evidenceRefs:z.array(dailyRecordIdSchema).min(1)}),
      z.object({state:z.literal('unknown'),reasonCode:dailyRecordIdSchema}),
    ]),
  })).length(6),
  interviews:z.array(z.object({field:z.enum(dailyRecordFields),reasonCode:dailyRecordIdSchema})),
  assessmentCodeTables:z.record(z.string(),z.array(z.string())).optional(),
  observationHandle:dailyRecordIdSchema.nullable().optional(),observationPersistence:z.enum(['owner_durable','process_local']).optional(),
}).refine(v=>new Set(v.fields.map(f=>f.field)).size===dailyRecordFields.length,'unique_intake_fields_required');

/** Compose existing read contracts, without accepting identity or proposals from the caller. */
export async function readDailyRecordByName(value:unknown,port:DailyRecordReadRoutePort,now:()=>number=Date.now){
  const input=withRequestIdentity('academy_dailyrecord_get',dailyRecordGetSchema.parse(value)),lessonDate=resolveAcademyDate(input.lessonDate,now());
  const base={displayName:input.displayName,lessonDate,timeZone:'Asia/Seoul',purpose:input.purpose,currentRequestRef:input.currentRequestRef,
    academyWrites:0,parentSends:0,automaticRetryAllowed:false,execution:'read_only',sourceContentIsUntrustedData:true};
  const held=(reasonCode:string)=>({...base,status:'unavailable' as const,sourceAvailability:'unavailable' as const,reasonCode});
  const heldException=(reasonCode:string,error:unknown)=>error instanceof UpstreamToolFailure
    ?{...held(reasonCode),failure:error.failure,...(error.upstreamOccurrence?{upstreamOccurrence:error.upstreamOccurrence}:{})}
    :held(reasonCode);
  let resolved:z.infer<typeof resolutionSchema>;
  try{resolved=resolutionSchema.parse(await port.resolveStudent({displayName:input.displayName,purpose:input.purpose,currentRequestRef:input.currentRequestRef,lessonDate}));}
  catch(error){return heldException('student_resolve_unqualified_no_automatic_retry',error);}
  if(resolved.status==='unavailable')return {...held(resolved.reasonCode),
    ...(resolved.failure?{failure:publicFailureFromData(resolved.failure)}:{}),
    ...(resolved.upstreamOccurrence?{upstreamOccurrence:resolved.upstreamOccurrence}:{})};
  if(resolved.displayName!==input.displayName)return held('student_name_scope_conflict');
  if(!resolved.teacherId)return held('dailyrecord_teacher_scope_unavailable');
  const context=dailyRecordContextSchema.parse({schemaVersion:'academy.dailyrecord-user-context/1',purpose:input.purpose,currentRequestRef:input.currentRequestRef,
    scope:{sourceNamespace:'academy.lms.dayrecord',teacherId:resolved.teacherId,studentId:resolved.studentKey,lessonDate},
    selectedFields:[...dailyRecordFields],memoAnswer:{kind:'not_supplied'},facts:[]});
  try{
    // Intake remains the source-qualification owner. No values, facts or memo answers are invented.
    const intake=intakeReadSchema.parse(await port.intake({context}));
    if(!intake||intake.sourceAvailability!=='observed'||!intake.source||intake.blockers.length){
      return {...held(intake?.blockers?.[0]??'dailyrecord_source_unqualified_no_automatic_retry'),intakeStatus:intake?.status??null};
    }
    if(JSON.stringify(intake.userContext.scope)!==JSON.stringify(context.scope))return held('dailyrecord_read_scope_conflict');
    const age=now()-Date.parse(intake.source.observedAt);
    if(age<0||age>300000)return held('stale_source');
    return {...base,status:'observed' as const,sourceAvailability:'observed' as const,scope:context.scope,
      route:['academy_student_resolve','academy_dailyrecord_intake'],fields:intake.fields,source:intake.source,
      preparation:{status:intake.status,interviews:intake.interviews,requiredOnlyForAuthoring:true},
      ...(intake.assessmentCodeTables?{assessmentCodeTables:intake.assessmentCodeTables}:{}),
      transmissionState:intake.source.transmissionState,
      // An owner-durable handle lets any later process review and save this exact
      // scope and request; a process-local handle would not outlive this call.
      ...(input.view==='compact'?{}:{authoringHandle:intake.observationPersistence==='owner_durable'&&intake.observationHandle
        ?{observationHandle:intake.observationHandle,persistence:'owner_durable' as const,boundTo:{scope:context.scope,currentRequestRef:context.currentRequestRef}}:null})};
  }catch(error){return heldException('dailyrecord_source_unqualified_no_automatic_retry',error);}
}
