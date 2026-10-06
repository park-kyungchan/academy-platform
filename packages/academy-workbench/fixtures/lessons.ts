import type { SyntheticLesson } from '../src/contracts/lesson-views.ts';
export const syntheticLessons: SyntheticLesson[] = [{
  id: 'synthetic-lesson-1', academyId: 'synthetic-academy', studentId: 'synthetic-student-1',
  context: { date: '2026-10-06', academyTimezone: 'Asia/Seoul', siteTimezone: 'unknown' },
  localStartTime: '16:00', assignmentId: 'assignment-1',
  material: { textbookEdition: 'synthetic-edition-2', unit: 'unit-1', section: 'section-A', problemRange: { first: 1, last: 5 }, videoVersion: 'synthetic-video-v7' },
  observations: { assignmentId: 'assignment-1', assignmentConfirmed: 'observed', appScoreObserved: 'not_checked', videoSubmitted: 'unresolved', teacherInspectionObserved: 'not_checked', paperWorkConfirmed: 'unresolved' },
  evidenceIds: ['evidence-1'], questionsToCheck: ['Check paper work'], preparation: ['Synthetic worksheet'],
  history: [{ assignmentId: 'synthetic-previous-assignment', note: 'Historical score observed; unrelated to current completion' }]
}];
