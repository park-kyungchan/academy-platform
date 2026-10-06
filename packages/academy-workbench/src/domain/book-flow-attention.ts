import type { RequirementFacts, DerivedAttention, HistoryEvent } from '../contracts/book-flow.ts';

/** DATA must already have resolved exact scope and explicit supersession. This pure
 * LOGIC layer neither collects evidence nor authorizes ACTION or invents history. */
export function deriveAttention(f: RequirementFacts, context: Pick<DerivedAttention, 'factIds' | 'coverageIds' | 'conflictIds' | 'asOf' | 'derivedAt'>): DerivedAttention & { includeInRequiredDenominator: boolean; flagMissingHomeApp: false } {
  const required = f.applicability === 'applicable' && f.assignment === 'assigned' && f.requiredness === 'required';
  const conflict = f.unresolvedConflict || f.completion === 'disputed' || f.evidencePresence === 'conflicting';
  const exact = f.relevantAssertionIsExact && f.supportingEvidenceIds.length > 0 && !conflict;
  const confirmedMissing = required && f.completion === 'confirmed_incomplete' && exact;
  const confirmedNeverStarted = confirmedMissing && f.effort === 'not_started_confirmed';
  const confirmedComplete = f.completion === 'confirmed_complete' && exact;
  const badges: string[] = [];
  if (f.applicability === 'not_applicable') badges.push('해당 없음');
  if (f.assignment === 'explicitly_unassigned') badges.push('미배정');
  if (f.requiredness === 'optional') badges.push('선택');
  if (confirmedNeverStarted) badges.push('미수행 확인');
  if (confirmedMissing) badges.push('확정 미완료');
  if (confirmedComplete) badges.push('완료 확인');
  if (conflict) badges.push('판정 확인 필요');
  if (required && !confirmedComplete && !confirmedMissing && f.deadline === 'past_due') badges.push('기한 경과 · 완료 미확인');
  if (f.coverage === 'not_queried') badges.push('미조회');
  if (f.coverage === 'blocked') badges.push('조회 제한');
  if (f.coverage === 'unsupported') badges.push('조회 미지원');
  if (f.coverage === 'partial' || f.evidencePresence === 'partial') badges.push('부분 근거');
  if (f.mapping !== 'exact') badges.push('매핑 확인 필요');
  if (f.deadline === 'unknown') badges.push('기한 미확인');
  if (f.applicability === 'unknown' || f.assignment === 'unknown' || f.requiredness === 'unknown') badges.push('범위 불명');
  if (!confirmedMissing && !confirmedComplete && !conflict && f.applicability !== 'not_applicable') badges.push('수행 여부 미확인');
  return { ...structuredClone(context), ruleVersion: 'book-flow-attention/1.0.0', headline: badges[0] ?? '수행 여부 미확인', badges,
    confirmedMissing, confirmedNeverStarted, includeInRequiredDenominator: required, flagMissingHomeApp: false,
    acceptedEvidenceIds: exact ? [...f.supportingEvidenceIds] : [],
    excludedEvidenceWithReasons: exact ? [] : f.supportingEvidenceIds.map(evidenceId => ({ evidenceId, reason: conflict ? 'Unresolved conflicting assertions' : 'No exact relevant assertion; query absence is not nonperformance' })) };
}

/** Unknown event time remains unknown. recordedAt is deliberately not a fallback. */
export function historyEventTime(event: HistoryEvent): string | null { return event.effectiveAt ?? event.occurredAt; }
