import type { ObservationState } from '../contracts/collection.ts';

/** Explicit synthetic policy: exemptions are not evidence of completed paper work. */
export type PaperException = { kind: 'none' } | { kind: 'excused'; teacherId: string; rationale: string };
export function paperStatus(observed: ObservationState, exception: PaperException): 'confirmed' | 'excused' | 'not_checked' | 'unresolved' {
  if (exception.kind === 'excused') {
    if (!exception.teacherId.trim() || !exception.rationale.trim()) throw new Error('Explicit teacher and rationale required');
    return 'excused';
  }
  return observed === 'observed' ? 'confirmed' : observed;
}

/** Structured storage prevents generated blocks from replacing human text. */
export interface Memo { userText: string; managedText: string; revision: number; }
export function mergeManagedMemo(base: Memo, current: Memo, generated: string):
  { ok: true; memo: Memo } | { ok: false; code: 'MANAGED_CONFLICT'; memo: Memo } {
  if (current.revision < base.revision || current.managedText !== base.managedText) {
    return { ok: false, code: 'MANAGED_CONFLICT', memo: structuredClone(current) };
  }
  return { ok: true, memo: { userText: current.userText, managedText: generated, revision: current.revision + 1 } };
}

export interface CoverageCounts { scope: 'recent' | 'all'; observed: number; completed: number; total: number | null; pageEndConfirmed: boolean; }
export function completionSummary(counts: CoverageCounts) {
  const { observed, completed, total, scope, pageEndConfirmed } = counts;
  if (![observed, completed].every(n => Number.isSafeInteger(n) && n >= 0) || completed > observed || (total !== null && (!Number.isSafeInteger(total) || total < observed))) throw new Error('Invalid coverage counts');
  const completeCoverage = scope === 'all' && pageEndConfirmed && total !== null && observed === total;
  return { scope, observed, completed, denominator: completeCoverage ? total : null,
    percent: completeCoverage && total > 0 ? completed / total * 100 : null,
    coverage: completeCoverage ? 'complete' as const : 'partial' as const };
}
