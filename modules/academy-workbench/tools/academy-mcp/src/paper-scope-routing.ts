import {z} from 'zod';

// Selected only from owner-controlled, release-pinned routing bytes. An
// upstream remains responsible for enforcing its own actual read scope.
export const paperScopesSchema=z.array(z.object({
  endpoint:z.enum(['paper1','paper2']),
  studentIds:z.array(z.string().regex(/^[1-9][0-9]{0,15}$/)).min(1).max(1000),
}).strict()).min(1).max(32).superRefine((entries,ctx)=>{
  if(new Set(entries.map(e=>e.endpoint)).size!==entries.length||entries.some(e=>new Set(e.studentIds).size!==e.studentIds.length))
    ctx.addIssue({code:'custom',message:'paper_scope_duplicate'});
});
export type PaperScopes=z.infer<typeof paperScopesSchema>;

const routable=new Set(['academy_paper_headers','academy_paper_definition_lookup','academy_printed_paper_lookup','academy_daily_test_inventory','academy_source_read_contracts','academy_paper_read_contracts']);
export interface PaperScopeDiagnostic{studentId:string;upstreamName:string;configuredEndpoints:string[];matchCount:number}
export class PaperScopeUnresolvedError extends Error{
  constructor(readonly diagnostic:PaperScopeDiagnostic){super('paper_scope_unresolved');}
}
/** Admission preflight: routing is owner-configured per student. A caller-supplied class, book or unit can never widen it. */
export function inspectPaperScope(scopes:PaperScopes|undefined,studentId:string){
  if(!scopes)return {status:'unconfigured' as const};
  const matches=scopes.filter(s=>s.studentIds.includes(studentId));
  return matches.length===1?{status:'resolved' as const,endpoint:matches[0]!.endpoint}
    :{status:'unresolved' as const,configuredEndpoints:scopes.map(s=>s.endpoint),matchCount:matches.length};
}
export function selectPaperRoute(scopes:PaperScopes|undefined,registry:any[],entry:any,args:unknown){
  if(!scopes||!['paper1','paper2'].includes(entry.endpoint)||!routable.has(entry.upstreamName))return entry;
  const studentId=z.object({studentId:z.string().regex(/^[1-9][0-9]{0,15}$/)}).passthrough().parse(args).studentId;
  const matches=scopes.filter(s=>s.studentIds.includes(studentId));
  const unresolved=()=>new PaperScopeUnresolvedError({studentId,upstreamName:entry.upstreamName,configuredEndpoints:scopes.map(s=>s.endpoint),matchCount:matches.length});
  if(matches.length!==1)throw unresolved();
  if(entry.endpoint==='paper2'&&matches[0]!.endpoint!=='paper2')throw unresolved();
  // Several public aliases may describe the same endpoint and upstream tool.
  // They are one dispatch target; their presentation metadata is not routing.
  const candidate=registry.find(r=>r.endpoint===matches[0]!.endpoint&&r.upstreamName===entry.upstreamName);
  if(!candidate)throw unresolved();
  return candidate;
}
