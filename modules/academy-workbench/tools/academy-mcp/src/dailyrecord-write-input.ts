import {randomUUID} from 'node:crypto';
import {z} from 'zod';
import {dailyRecordContextSchema,dailyRecordFields,type DailyRecordContext,type DailyRecordField,type DailyRecordScope} from './dailyrecord-context.ts';
import {userRequestSchema,type DailyRecordUserRequest} from './dailyrecord-workflow.ts';

/**
 * Defaults of the DailyRecord write path (intake, review, save). A write
 * needs only the observation handle, the new field text and, at save, the
 * user's words. Every other member is optional; a member the caller gives
 * keeps its meaning.
 */
export const flexibleContextSchema=z.object(dailyRecordContextSchema.shape).partial().strict();
export type FlexibleContext=z.infer<typeof flexibleContextSchema>;
export const DEFAULT_WRITE_PURPOSE='DailyRecord write';

type Decision={kind:'set'|'replace'|'clear';requestRef:string};
export type FlexibleProposal={field:DailyRecordField;value:string;factRefs?:string[];decision?:{kind:Decision['kind'];requestRef?:string}};
export type ResolvedProposal<P extends FlexibleProposal=FlexibleProposal>=Omit<P,'factRefs'|'decision'>&{factRefs:string[];decision:Decision};
type Before={state:'known';value:string}|{state:'unknown'}|undefined;

const memoText=dailyRecordContextSchema.shape.memoAnswer.options[2].shape.text;
/** The full context: the caller's members, else the handle's scope and request, else the defaults. */
export function resolveWriteContext(context:FlexibleContext|undefined,proposals:readonly FlexibleProposal[],bound?:{scope:DailyRecordScope;requestRef:string}):DailyRecordContext{
  const scope=context?.scope??bound?.scope;
  if(!scope)throw Error('observation_handle_or_context_scope_required');
  const currentRequestRef=context?.currentRequestRef??bound?.requestRef??'dailyrecord-request:'+randomUUID();
  // The proposed memo is the memo answer; no proposed memo adds nothing; a blank one is asked for.
  const memo=proposals.find(p=>p.field==='lessonMemo');
  const memoAnswer=context?.memoAnswer??(!memo?{kind:'no_addition' as const,requestRef:currentRequestRef}
    :memoText.safeParse(memo.value).success?{kind:'content' as const,requestRef:currentRequestRef,text:memo.value}:{kind:'not_supplied' as const});
  const proposed=[...new Set(proposals.map(p=>p.field))];
  return dailyRecordContextSchema.parse({schemaVersion:context?.schemaVersion??'academy.dailyrecord-user-context/1',purpose:context?.purpose??DEFAULT_WRITE_PURPOSE,
    currentRequestRef,scope,selectedFields:context?.selectedFields??(proposed.length?proposed:[...dailyRecordFields]),memoAnswer,facts:context?.facts??[]});
}

/** An empty, blank or unknown-assessment current value takes set; any other value is replaced, whatever kind the caller wrote. Clear stays explicit. */
export function resolveProposal<P extends FlexibleProposal>(p:P,context:DailyRecordContext,before:Before):ResolvedProposal<P>{
  const filled=before?.state==='known'&&before.value.trim()!==''&&!(['dailyTestAssessment','previousHomeworkAssessment'].includes(p.field)&&before.value==='-1');
  const kind=p.decision?.kind==='clear'?'clear':filled?'replace':p.decision?.kind??'set';
  return {...p,factRefs:p.factRefs??context.facts.filter(f=>f.field===p.field).map(f=>f.ref),
    decision:{kind,requestRef:p.decision?.requestRef??context.currentRequestRef}};
}

/** Fact checks report, never block: the teacher's words at save are the authority for the text. A supplied fact that contradicts a code value blocks instead (dailyrecord.ts codeFactBlockers). */
export const FACT_WARNING_TEXT:Record<string,string>={
  supplied_fact_fragment_missing:'A required fragment of the supplied facts is not in the text.',
  unsupported_question_count:'A question count in the text is not in the supplied facts.',
  count_rendering_must_preserve_count:'A questionCounts rendering does not state its count (8문항); ordinals such as 유형 10 belong in requiredFragments.',
  unsupported_deadline:'A deadline in the text is not in the supplied facts.',
  unsupported_completion_claim:'The text says 완료, but no supplied fact does.',
  unsupported_stage_completion_claim:'The text says a stage is complete that no supplied fact confirms.',
  stage_completion_mismatch:'A supplied fact says this stage is not complete.',
  explicit_user_source_option_fact_required:'No supplied fact gives this code value.',
};
export const factWarnings=(codes:readonly string[])=>[...new Set(codes)].map(code=>({code,text:FACT_WARNING_TEXT[code.split(':')[0]!]??code}));

/** A save instruction: the user's words, as a string or as {text}. */
export const saveInstructionSchema=z.union([z.string().max(4000),userRequestSchema.shape.instruction.extend({text:z.string().max(4000)})]);
/** The user request behind a save, or null when the words are empty. */
export function requestFromInstruction(instruction:z.infer<typeof saveInstructionSchema>,requestRef:string):DailyRecordUserRequest|null{
  const value=typeof instruction==='string'?{text:instruction}:instruction;
  return value.text.trim()?userRequestSchema.parse({requestRef,instruction:value}):null;
}
