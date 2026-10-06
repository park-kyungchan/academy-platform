import type { ActionContract,LogicContract } from '../contracts/book-flow.ts';
export const SYNC_LOGIC:LogicContract={
  ruleId:'exact-allowlist-pagination-revision',version:'1.0.0',practicalContextIds:['synthetic-date-first-preparation'],
  rationale:'Keep exact query scope, source observations, completion facts and teacher interpretation separate.',
  inputDataContextIds:['runtime-scoped-sync-context'],inputSchemaRef:'sync/SourcePage',outputSchemaRef:'sync/ScopeCoverage',
  uncertaintyPolicy:['No verified empty until pagination and total closure','Unknown source timezone and event time stay null','Failed refresh remains stale','Source observations never assert teacher completion'],sourceIds:['synthetic-contract']
};
export const SYNC_ACTION:ActionContract={
  id:'synthetic-scoped-read/v1',version:'1.0.0',practicalContextIds:['synthetic-date-first-preparation'],
  intent:'Collect minimized observations from an authorized synthetic batch',dataContextIds:['runtime-scoped-sync-context'],logicRuleIds:[SYNC_LOGIC.ruleId],targetSchemaRef:'sync/SyncPlan',
  preconditions:['Trusted target and source-kind allowlists','Synthetic adapter only','Exclusive in-memory resource lock'],
  sideEffects:['Append minimized local synthetic observation revisions, coverage, checkpoints and action audit'],
  authorization:{evaluatedAtRuntime:true,allowedScopeRequired:true,explicitAuthorityForEffects:true,documentGrantsPermission:false},
  idempotency:{strategy:'Request and plan identity, cursor checkpoint, content hash and monotonic source revision',uncertainOutcomePolicy:'Resume the same failed read snapshot; restart with a new request if the source snapshot changed'},
  verification:['Exact student and source scope','Gap-free page chain and unique total','No raw text persisted','Teacher notes preserved'],auditEventType:'ActionAttempt',
  webMcp:{toolName:'get_synthetic_sync_result',registrationState:'not_verified',executionImplemented:false}
};
