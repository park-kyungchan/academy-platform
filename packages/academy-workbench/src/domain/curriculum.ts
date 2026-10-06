import { SECTION_REGISTRY, type BookIdentity, type StudentBookMap, type SectionRequirement } from '../contracts/curriculum.ts';
import { deriveAttention } from './book-flow-attention.ts';
export type ProgressState = 'complete' | 'missing' | 'incomplete' | 'unqueried' | 'unassigned' | 'not_applicable' | 'scope_unknown' | 'partial' | 'stale' | 'unresolved' | 'blocked' | 'unsupported';
export const STATE_LABEL: Record<ProgressState, string> = { complete: '완료 근거', missing: '미수행 확인', incomplete: '확정 미완료', unqueried: '미조회', unassigned: '미배정', not_applicable: '해당 없음', scope_unknown: '범위 불명', partial: '부분 확인', stale: '오래된 근거', unresolved: '근거 불충분', blocked:'조회 제한', unsupported:'조회 미지원' };
export function bookDisplayName(book: BookIdentity) { return `${book.series} ${book.grade ?? '?'}-${book.semester ?? '?'}-${book.volume ?? '?'}`; }
export function currentEvidence(requirement: SectionRequirement) {
  const assignedAt = Date.parse(requirement.assignedAt ?? '');
  return requirement.evidence.filter(e => e.assignmentId === requirement.assignmentId && e.kind === (requirement.completionAuthority === 'teacher' ? 'teacher_confirmation' : requirement.channel === 'video' ? 'video_path' : 'problem_grading') && (requirement.completionAuthority !== 'teacher' || !!e.teacherId?.trim()) && Number.isFinite(assignedAt) && Number.isFinite(Date.parse(e.effectiveAt ?? e.occurredAt ?? '')) && Date.parse(e.effectiveAt ?? e.occurredAt ?? '') >= assignedAt);
}
export function requirementAttention(r: SectionRequirement, asOf = '2026-10-06T08:00:00Z') {
  const evidence = currentEvidence(r);
  const conflict = new Set(evidence.map(e=>e.outcome)).size > 1;
  const exact = !r.stale && r.axes.mapping === 'exact' && evidence.length > 0 && !conflict && evidence.every(e=>e.outcome === (r.axes.completion === 'confirmed_complete' ? 'complete' : 'missing'));
  // This adapter is for the fixed synthetic snapshot; callers may pass its explicit as-of.
  return deriveAttention({ ...r.axes, deadline:'unknown', evidencePresence:conflict?'conflicting':evidence.length?'present':'unknown', supportingEvidenceIds:evidence.map(e=>e.id), relevantAssertionIsExact:exact, unresolvedConflict:conflict, homeAppRequired:null, dueDate:null, dueAt:null, dueTimezone:null, cutoffPolicy:null }, {factIds:[r.id], coverageIds:[], conflictIds:conflict?evidence.map(e=>e.id):[], asOf, derivedAt:asOf});
}
export function requirementState(r: SectionRequirement): ProgressState {
  if (r.axes.applicability === 'not_applicable') return 'not_applicable';
  if (r.axes.applicability === 'unknown' || r.axes.assignment === 'unknown' || r.axes.mapping !== 'exact') return 'scope_unknown';
  if (r.axes.assignment === 'explicitly_unassigned') return 'unassigned';
  if (!r.assignmentId) return 'scope_unknown';
  if (!r.assignedAt || !Number.isFinite(Date.parse(r.assignedAt))) return 'scope_unknown';
  const attention = requirementAttention(r);
  if (attention.confirmedMissing) return attention.confirmedNeverStarted ? 'missing' : 'incomplete';
  if (r.axes.coverage === 'blocked' || r.axes.coverage === 'unsupported') return r.axes.coverage;
  if (r.axes.coverage === 'not_queried') return 'unqueried';
  if (r.stale) return 'stale';
  const evidence = currentEvidence(r);
  if (!evidence.length) return 'unresolved';
  if (new Set(evidence.map(e => e.outcome)).size !== 1) return 'unresolved';
  if (r.axes.completion === 'disputed') return 'unresolved';
  if (r.axes.coverage === 'partial' || evidence[0]!.outcome === 'partial') return 'partial';
  if (evidence[0]!.outcome === 'missing') return r.axes.completion === 'confirmed_incomplete' ? 'incomplete' : 'unresolved';
  return r.axes.completion === 'confirmed_complete' ? 'complete' : 'unresolved';
}
export function sectionState(rows: readonly SectionRequirement[]): ProgressState {
  if (!rows.length) return 'scope_unknown';
  const states = rows.map(requirementState).filter(s => s !== 'not_applicable');
  if (!states.length) return 'not_applicable';
  // A confirmed missing requirement is visible even if a separate channel is unresolved.
  for (const state of ['missing','incomplete','scope_unknown','blocked','unsupported','unqueried','stale','unresolved','partial','unassigned'] as const) if (states.includes(state)) return state;
  return 'complete';
}
export function progressSummary(map: StudentBookMap) {
  const assigned = map.requirements.filter(r => r.axes.applicability === 'applicable' && r.axes.assignment === 'assigned' && r.axes.requiredness === 'required' && r.assignmentId !== null);
  const completed = assigned.filter(r => requirementState(r) === 'complete').length;
  const covered = assigned.filter(r => r.axes.coverage === 'complete' && !r.stale).length;
  const resolved = assigned.filter(r => ['complete','missing','incomplete','partial'].includes(requirementState(r))).length;
  const missingDenominator = assigned.filter(r => requirementAttention(r).confirmedMissing || requirementAttention(r).badges.includes('완료 확인')).length;
  const denominatorKnown = map.assignmentInventoryComplete && !map.requirements.some(r => r.axes.applicability === 'unknown' || r.axes.assignment === 'unknown' || r.axes.requiredness === 'unknown' || r.axes.mapping !== 'exact') && !map.units.some(u => u.level === 'unknown' || u.parentId === 'unknown' || u.sectionCoverage !== 'complete');
  return { assigned: assigned.length, completed, covered, resolved, missingDenominator, missing: assigned.filter(r => requirementAttention(r).confirmedMissing).length, denominatorKnown,
    percent: denominatorKnown && covered === assigned.length && resolved === assigned.length && assigned.length > 0 ? Math.round(completed / assigned.length * 100) : null };
}
export function validateBookMap(map: StudentBookMap): void {
  const assert = (condition: boolean, message: string) => { if (!condition) throw new Error(message); };
  assert(map.source === 'synthetic', 'Synthetic curriculum only');
  assert([map.book.grade, map.book.semester, map.book.volume].every(n => n === null || Number.isSafeInteger(n) && n > 0), 'Book identity numbers');
  assert((map.book.semester === null || map.book.semester <= 2) && !!map.book.series.trim() && !!map.book.nativeBookCode.trim() && !!map.book.version.trim(), 'Book identity fields');
  for (const rows of [map.units, map.sections, map.requirements]) assert(new Set(rows.map(r => r.id)).size === rows.length, 'Duplicate curriculum IDs');
  for (const unit of map.units) {
    const seen = new Set<string>([unit.id]);
    let ancestor = map.units.find(u => u.id === unit.parentId);
    while (ancestor) { assert(!seen.has(ancestor.id), 'Hierarchy cycle'); seen.add(ancestor.id); ancestor = map.units.find(u => u.id === ancestor!.parentId); }
    if (unit.level === 'unknown' || unit.parentId === 'unknown') continue;
    const parent = map.units.find(u => u.id === unit.parentId);
    assert(unit.level === 'major_unit' ? unit.parentId === null : !!parent && (unit.level === 'middle_unit' ? parent.level === 'major_unit' : parent.level === 'major_unit' || parent.level === 'middle_unit'), 'Invalid hierarchy');
  }
  for (const section of map.sections) {
    const scope = map.units.find(u => u.id === section.scopeUnitId);
    assert(!!scope && (SECTION_REGISTRY[map.book.schoolLevel] as readonly string[]).includes(section.name), 'Section scope');
    assert(section.name !== '대단원 총괄평가' || scope?.level === 'major_unit', 'Major assessment scope');
  }
  assert(new Set(map.sections.map(s => `${s.scopeUnitId}:${s.name}`)).size === map.sections.length, 'Duplicate scoped section');
  for (const r of map.requirements) {
    const section = map.sections.find(s => s.id === r.sectionId);
    assert(!!section, 'Orphan requirement');
    assert(section?.name !== '개념 백지 테스트' || r.completionAuthority === 'teacher', 'Blank test requires teacher completion authority');
  }
}
