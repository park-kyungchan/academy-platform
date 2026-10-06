import { expect, test } from 'bun:test';
import { createCurriculumFixture } from '../fixtures/curriculum.ts';
import { createStudentRepository } from '../fixtures/student-repository.ts';
import { bookDisplayName, currentEvidence, requirementState, sectionState, progressSummary, validateBookMap } from '../src/domain/curriculum.ts';
import { SECTION_REGISTRY } from '../src/contracts/curriculum.ts';
import { historyState, filterHistory } from '../src/domain/history.ts';
import { SyntheticApplicationService } from '../src/application/service.ts';
import { createUIFixture } from '../fixtures/ui.ts';
import { CONTRACT_REVISION } from '../src/domain/canonical.ts';
const books = () => createCurriculumFixture('synthetic-student-1');

test('book-flow v1 names preserve separate identity and unknown numbers', () => {
  const book = books()[0]!.book;
  expect(bookDisplayName(book)).toBe('가우스 5-2-2');
  expect(bookDisplayName({ ...book, grade: null, volume: null })).toBe('가우스 ?-2-?');
  expect(book.originalLabel).toContain('합성 원명');
  expect(book.id).not.toBe(bookDisplayName(book));
});
test('school registries preserve exact elementary nine and middle eight labels', () => {
  expect(SECTION_REGISTRY.elementary).toHaveLength(9); expect(SECTION_REGISTRY.middle).toHaveLength(8);
  expect(SECTION_REGISTRY.elementary).toContain('필수예제'); expect(SECTION_REGISTRY.middle).toContain('필수 예제');
  expect(books()[1]!.sections[0]).toMatchObject({name:'개념 백지 테스트',sourceLabel:'SYN_SOURCE_blank_review',mappingStatus:'unverified'});
});
test('hierarchy accepts omitted middle and unknown levels without fabricating instances', () => {
  const map=books()[0]!; expect(()=>validateBookMap(map)).not.toThrow();
  expect(map.sections.filter(s=>s.scopeUnitId==='minor-1')).toHaveLength(4);
  expect(map.sections.find(s=>s.name==='대단원 총괄평가')!.scopeUnitId).toBe('major-1');
  map.units.push({id:'unknown-node',parentId:'unknown',level:'unknown',title:'구조 미확인',order:20});
  expect(()=>validateBookMap(map)).not.toThrow(); expect(progressSummary(map).denominatorKnown).toBe(false);
});
test('hierarchy rejects misplaced assessment cycles and mixed school vocabulary', () => {
  const map=books()[0]!; map.sections.find(s=>s.name==='대단원 총괄평가')!.scopeUnitId='minor-1';
  expect(()=>validateBookMap(map)).toThrow('Major assessment scope');
  const cycle=books()[0]!; cycle.units[0]!.parentId='middle-1';
  expect(()=>validateBookMap(cycle)).toThrow('Hierarchy cycle');
  const mixed=books()[0]!; mixed.sections[0]!.name='개념 백지 테스트';
  expect(()=>validateBookMap(mixed)).toThrow('Section scope');
});
test('exact incomplete assertion remains known independently of query coverage', () => {
  const r=books()[0]!.requirements.find(r=>r.id==='s1-problem')!;
  expect(requirementState(r)).toBe('missing');
  expect(requirementState({...r,evidence:[]})).toBe('unresolved');
  expect(requirementState({...r,assignmentId:null,axes:{...r.axes,assignment:'explicitly_unassigned'}})).toBe('unassigned');
  expect(requirementState({...r,axes:{...r.axes,coverage:'not_queried'}})).toBe('missing');
  expect(requirementState({...r,axes:{...r.axes,coverage:'partial'}})).toBe('missing');
  expect(requirementState({...r,stale:true})).toBe('stale');
});
test('all uncertainty states remain distinct from missing and completion', () => {
  const states=new Set(books()[0]!.requirements.map(requirementState));
  for(const state of ['complete','missing','unqueried','unassigned','not_applicable','scope_unknown','partial','stale','unresolved']) expect(states.has(state as ReturnType<typeof requirementState>)).toBe(true);
});
test('historical score wrong channel and conflicting evidence cannot complete new assignment', () => {
  const map=books()[0]!, old=map.requirements.find(r=>r.id==='s8-problem')!;
  expect(currentEvidence(old)).toHaveLength(0); expect(requirementState(old)).toBe('unresolved');
  const video=map.requirements[0]!;
  expect(requirementState({...video,evidence:video.evidence.map(e=>({...e,kind:'problem_grading'}))})).toBe('unresolved');
  expect(requirementState({...video,evidence:[...video.evidence,{...video.evidence[0]!,id:'conflict',outcome:'missing'}]})).toBe('unresolved');
});
test('middle blank test requires explicit teacher completion evidence', () => {
  const r=books()[1]!.requirements[0]!;
  expect(requirementState(r)).toBe('unresolved');
  const confirmation={...r.evidence[0]!,kind:'teacher_confirmation' as const,teacherId:'synthetic-teacher'};
  expect(requirementState({...r,evidence:[confirmation],axes:{...r.axes,completion:'confirmed_complete'}})).toBe('complete');
  expect(requirementState({...r,evidence:[{...confirmation,teacherId:''}]})).toBe('unresolved');
  const invalid=books()[1]!; invalid.requirements[0]!.completionAuthority='evidence'; expect(()=>validateBookMap(invalid)).toThrow('teacher');
});
test('section keeps video complete and problem missing evidence separate', () => {
  const rows=books()[0]!.requirements.filter(r=>r.sectionId==='s1');
  expect(rows.map(requirementState)).toEqual(['complete','missing']); expect(sectionState(rows)).toBe('missing');
});
test('completion denominator and coverage are independent and never imply mastery', () => {
  const map=books()[0]!, summary=progressSummary(map);
  expect(summary.assigned).toBeGreaterThan(summary.covered); expect(summary.percent).toBeNull(); expect(summary.denominatorKnown).toBe(false);
  map.requirements=[map.requirements[0]!]; map.assignmentInventoryComplete=true;
  map.units.forEach(u=>{u.sectionCoverage='complete';});
  expect(progressSummary(map)).toMatchObject({assigned:1,completed:1,covered:1,percent:100});
  expect(progressSummary(books()[2]!)).toMatchObject({assigned:0,percent:null});
});
test('repository and shared service isolate student book maps and history', () => {
  const repo=createStudentRepository(), f=createUIFixture();
  expect(repo.readStudentBooks('other','synthetic-student-1')).toEqual([]);
  const s=new SyntheticApplicationService(f.snapshot,f.lessons,[],repo);
  const req={action:'studentLessonDetail' as const,academyId:'synthetic-academy',contractsRevision:CONTRACT_REVISION,idempotencyKey:'book-1',params:{context:f.lessons[0]!.context,studentId:'synthetic-student-1'}};
  const result=s.execute({actorId:'synthetic-viewer',role:'viewer',academyIds:['synthetic-academy']},req);
  expect(result.ok).toBe(true);
  if(result.ok){expect(result.data.books).toHaveLength(3);expect(result.data.timeline.every(e=>e.studentId==='synthetic-student-1')).toBe(true);result.data.books[0]!.book.series='mutated';}
  expect(repo.readStudentBooks('synthetic-academy','synthetic-student-1')[0]!.book.series).toBe('가우스');
  expect(s.execute({actorId:'synthetic-viewer',role:'viewer',academyIds:[]},req)).toMatchObject({ok:false,code:'FORBIDDEN'});
});
test('history filters separate confirmed missing from overdue unconfirmed and partial evidence', () => {
  const entries=createStudentRepository().readStudentHistory('synthetic-academy','synthetic-student-1');
  const now='2026-10-06T08:00:00Z';
  expect(filterHistory(entries,'missing',now).length).toBeGreaterThan(0);
  expect(filterHistory(entries,'overdue',now).every(e=>historyState(e,now).state!=='missing')).toBe(true);
  expect(filterHistory(entries,'unqueried',now).length).toBeGreaterThan(0);
  expect(filterHistory(entries,'partial',now).length).toBeGreaterThan(0);
  const old=entries.find(e=>e.requirement.id==='s8-problem')!;
  expect(historyState(old,now).state).toBe('unresolved'); expect(entries.every(e=>e.teacherJudgment===null)).toBe(true);
});
test('seven axes retain optional unknown blocked unsupported and disputed independently', () => {
  const r=books()[0]!.requirements[0]!;
  expect(requirementState({...r,axes:{...r.axes,coverage:'blocked'}})).toBe('blocked');
  expect(requirementState({...r,axes:{...r.axes,coverage:'unsupported'}})).toBe('unsupported');
  expect(requirementState({...r,axes:{...r.axes,assignment:'unknown'}})).toBe('scope_unknown');
  expect(requirementState({...r,axes:{...r.axes,completion:'disputed'}})).toBe('unresolved');
  const map=books()[0]!;map.requirements=[{...r,axes:{...r.axes,requiredness:'optional'}}];
  expect(progressSummary(map)).toMatchObject({assigned:0,percent:null});
});
test('incomplete is not unperformed without explicit effort evidence', () => {
  const r=books()[0]!.requirements.find(r=>r.id==='s1-problem')!;
  expect(requirementState({...r,axes:{...r.axes,effort:'unknown'}})).toBe('incomplete');
  expect(requirementState({...r,axes:{...r.axes,effort:'started_confirmed'}})).toBe('incomplete');
});
test('query coverage can be complete while completion is disputed', () => {
  const map=books()[0]!;const r=map.requirements[0]!;
  map.requirements=[{...r,axes:{...r.axes,completion:'disputed'}}];
  expect(progressSummary(map)).toMatchObject({assigned:1,covered:1,completed:0,resolved:0,percent:null});
});
