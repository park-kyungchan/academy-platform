import type { StudentHistoryEntry } from '../contracts/student-history.ts';
import { requirementState } from './curriculum.ts';
export function historyState(entry: StudentHistoryEntry, asOf: string) {
  const state = requirementState(entry.requirement);
  const overdue = entry.requirement.assignmentId !== null && entry.requirement.axes.assignment === 'assigned' && entry.requirement.axes.applicability === 'applicable' && entry.requirement.axes.requiredness === 'required' && entry.dueAt !== null && /T.*(?:Z|[+-]\d{2}:\d{2})$/.test(entry.dueAt) && Date.parse(entry.dueAt) < Date.parse(asOf) && !['complete','not_applicable','unassigned','missing','incomplete'].includes(state);
  return { state, overdue, label: overdue ? '기한 경과 · 완료 미확인' : null };
}
export function filterHistory(entries: readonly StudentHistoryEntry[], filter: string, asOf: string) {
  return entries.filter(e => { const s = historyState(e,asOf); return filter === 'all' || filter === 'overdue' && s.overdue || filter === s.state; }).sort((a,b)=>b.assignedAt.localeCompare(a.assignedAt)||a.id.localeCompare(b.id));
}
