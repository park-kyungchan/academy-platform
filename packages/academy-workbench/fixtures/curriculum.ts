import type { StudentBookMap, SectionRequirement, StatusAxes } from '../src/contracts/curriculum.ts';
const axes = (variant: string): StatusAxes => ({ applicability:variant==='not_applicable'?'not_applicable':variant==='scope_unknown'?'unknown':'applicable', assignment:variant==='unassigned'||variant==='not_applicable'?'explicitly_unassigned':variant==='scope_unknown'?'unknown':'assigned', requiredness:'required', completion:variant==='complete'?'confirmed_complete':variant==='missing'||variant==='partial'?'confirmed_incomplete':'unknown', effort:variant==='missing'?'not_started_confirmed':variant==='complete'||variant==='partial'?'started_confirmed':'unknown', mapping:variant==='scope_unknown'?'unresolved':'exact', coverage:variant==='unqueried'?'not_queried':variant==='partial'?'partial':'complete' });
/** Section vocabulary supplied by user; all unit titles, placements and student progress below are fabricated. */
export function createCurriculumFixture(studentId: string): StudentBookMap[] {
  const map: StudentBookMap = {
    academyId: 'synthetic-academy', studentId, source: 'synthetic', sourceRevision: 'synthetic-curriculum-1', observedAt: '2026-10-06T05:30:00Z', assignmentInventoryComplete: true,
    book: { id: 'gauss-5-2-2', series: '가우스', grade: 5, semester: 2, volume: 2, originalLabel: '합성 원명: 초등 가우스 5학년 2학기 제2권', schoolLevel: 'elementary', nativeBookCode: 'SYN-GAUSS-522', version: '합성 2026 판본 A' },
    units: [
      { id: 'major-1', parentId: null, level: 'major_unit', title: '1. 분수의 곱셈', order: 1 },
      { id: 'middle-1', parentId: 'major-1', level: 'middle_unit', title: '1-1. 곱셈의 의미', order: 2 },
      { id: 'minor-1', parentId: 'middle-1', level: 'subunit', title: '분수와 자연수', order: 3 },
      { id: 'minor-2', parentId: 'middle-1', level: 'subunit', title: '분수끼리의 곱셈', order: 4 },
      { id: 'major-2', parentId: null, level: 'major_unit', title: '2. 소수의 곱셈', order: 5 },
      { id: 'minor-3', parentId: 'major-2', level: 'subunit', title: '소수와 자연수', order: 6 }
    ],
    sections: [
      { id: 's1', scopeUnitId: 'minor-1', name: '개념 이해하기' },
      { id: 's2', scopeUnitId: 'minor-1', name: '개념정복' },
      { id: 's3', scopeUnitId: 'minor-1', name: '필수예제' },
      { id: 's4', scopeUnitId: 'minor-1', name: '4M 듣기와 쓰기' },
      { id: 's5', scopeUnitId: 'minor-2', name: '유형다지기' },
      { id: 's6', scopeUnitId: 'minor-2', name: '실력쌓기' },
      { id: 's7', scopeUnitId: 'minor-2', name: '개념복습' },
      { id: 's8', scopeUnitId: 'minor-2', name: '4M 읽기와 쓰기' },
      { id: 's9', scopeUnitId: 'major-1', name: '대단원 총괄평가' },
      { id: 's10', scopeUnitId: 'minor-3', name: '개념 이해하기' },
      { id: 's11', scopeUnitId: 'minor-3', name: '필수예제' },
      { id: 's12', scopeUnitId: 'major-2', name: '대단원 총괄평가' }
    ], requirements: []
  };
  const add = (sectionId: string, channel: 'video' | 'problem', variant: 'complete' | 'missing' | 'unqueried' | 'unassigned' | 'not_applicable' | 'scope_unknown' | 'partial' | 'stale' | 'historical') => {
    const id = `${sectionId}-${channel}`, assignmentId = `${studentId}-${id}-new`;
    const row: SectionRequirement = { id, sectionId, channel, axes:axes(variant), assignmentId: variant === 'unassigned' || variant === 'not_applicable' ? null : assignmentId,
      assignedAt: Number(sectionId.slice(1)) <= 4 ? '2026-10-05T07:00:00Z' : Number(sectionId.slice(1)) <= 8 ? '2026-10-04T07:00:00Z' : '2026-10-03T07:00:00Z',
      stale: variant === 'stale', evidence: [],
      reason: ({ complete: '이번 배정 요구에 대한 합성 완료 근거가 있습니다. 지식 숙달 판정은 아닙니다.', missing: '이번 배정 요구를 조회했고 미수행으로 확인한 합성 근거가 있습니다.', unqueried: '배정은 확인했지만 수행 근거는 아직 조회하지 않았습니다.', unassigned: '현재 배정된 요구가 없습니다.', not_applicable: '이 섹션에는 이 유형의 요구가 적용되지 않습니다.', scope_unknown: '현재 배정 대상 범위가 확정되지 않았습니다.', partial: '일부 범위만 조회했습니다. 전체 수행 여부를 알 수 없습니다.', stale: '조회 근거가 오래되어 현재 상태를 확정할 수 없습니다.', historical: '다른 과제의 배정 이전 기록만 있습니다. 이번 과제 완료 근거에서 제외합니다.' })[variant] };
    if (!['unqueried','unassigned','not_applicable','scope_unknown'].includes(variant)) row.evidence.push({ id: `ev-${studentId}-${id}`, assignmentId: variant === 'historical' ? 'old-assignment' : assignmentId,
      kind: channel === 'video' ? 'video_path' : 'problem_grading', observedAt: variant === 'historical' ? '2026-10-01T05:00:00Z' : '2026-10-06T05:30:00Z', outcome: variant === 'missing' ? 'missing' : variant === 'partial' ? 'partial' : 'complete', sourceReference: `fixture:curriculum/${id}` });
    map.requirements.push(row);
  };
  add('s1','video','complete'); add('s1','problem','missing');
  add('s2','problem','complete'); add('s3','problem','unqueried'); add('s4','video','partial');
  add('s5','problem','unassigned'); add('s6','problem','scope_unknown'); add('s7','problem','not_applicable'); add('s8','problem','historical');
  add('s9','problem','missing'); add('s10','video','stale'); add('s11','problem','complete'); add('s12','problem','unassigned');
  if (studentId.endsWith('2')) { map.requirements.find(r => r.id === 's1-problem')!.axes.coverage = 'not_queried'; map.assignmentInventoryComplete = false; }
  if (studentId.endsWith('3')) map.requirements.find(r => r.id === 's3-problem')!.axes.coverage = 'partial';
  const long = structuredClone(map);
  long.book = { ...long.book, id: 'gauss-extension-5-2-1', series: '가우스 사고력 확장과 심화 탐구를 위한 아주 긴 합성 교재명', volume: 1, originalLabel: '합성 원명: 사고력 확장 특별판 (사용자 화면 줄바꿈 검증용)', nativeBookCode: 'SYN-LONG-521', version: '합성 2026 판본 B' };
  long.requirements = long.requirements.map(r => ({ ...r, axes:axes('unassigned'), assignmentId: null, evidence: [], stale: false, reason: '새 교재이며 아직 배정하지 않았습니다.' }));
  map.units.forEach(u => { u.sectionCoverage = u.id === 'minor-2' ? 'partial' : 'complete'; });
  map.sections.forEach(s => { s.sourceLabel = null; s.mappingStatus = 'unverified'; });
  const middle = structuredClone(map);
  middle.book = { id: 'gauss-middle-1-1-1', series: '가우스', grade: 1, semester: 1, volume: 1, originalLabel: '합성 원명: 중등 가우스 1학년 1학기 1권', schoolLevel: 'middle', nativeBookCode: 'SYN-M-GAUSS-111', version: '합성 중등 판본 2026' };
  middle.units = [{ id: 'm-major', parentId: null, level: 'major_unit', title: '1. 수와 연산', order: 1, sectionCoverage: 'complete' }, { id: 'm-minor', parentId: 'm-major', level: 'subunit', title: '정수와 유리수', order: 2, sectionCoverage: 'partial' }];
  middle.sections = [
    { id: 'm1', scopeUnitId: 'm-minor', name: '개념 백지 테스트', sourceLabel: 'SYN_SOURCE_blank_review', mappingStatus: 'unverified' },
    { id: 'm2', scopeUnitId: 'm-minor', name: '개념 예제', sourceLabel: null, mappingStatus: 'unverified' },
    { id: 'm3', scopeUnitId: 'm-minor', name: '개념 쏙 생각 쑥' },
    { id: 'm4', scopeUnitId: 'm-minor', name: '기본 문제' },
    { id: 'm5', scopeUnitId: 'm-minor', name: '필수 예제' },
    { id: 'm6', scopeUnitId: 'm-minor', name: '유형 다지기' },
    { id: 'm7', scopeUnitId: 'm-minor', name: 'Daily Restudy' },
    { id: 'm8', scopeUnitId: 'm-major', name: '대단원 총괄평가' }
  ];
  middle.requirements = middle.sections.map((s, i) => ({ id: `${s.id}-problem`, sectionId: s.id, channel: 'problem', axes:axes(i===0?'historical':i===2?'unqueried':i===3?'missing':'complete'), completionAuthority: i === 0 ? 'teacher' : 'evidence', assignmentId: `${studentId}-${s.id}-new`, assignedAt: '2026-10-05T07:00:00Z', stale: false,
    reason: i === 0 ? '합성 앱 채점 기록은 있지만 강사 완료판정은 없습니다. 자동 완료 처리하지 않습니다.' : '중등 합성 진행 예시입니다. 실제 섹션 배치는 확인되지 않았습니다.',
    evidence: i === 2 ? [] : [{ id: `ev-${studentId}-${s.id}`, assignmentId: `${studentId}-${s.id}-new`, kind: 'problem_grading', observedAt: '2026-10-06T05:30:00Z', outcome: i === 3 ? 'missing' : 'complete', sourceReference: `fixture:middle/${s.id}` }] }));
  // Explicit synthetic event times, never inferred from an imported observation time.
  for (const book of [map, middle, long]) for (const r of book.requirements) for (const e of r.evidence) {
    e.occurredAt = e.assignmentId === 'old-assignment' ? '2026-10-01T04:00:00Z' : '2026-10-06T04:00:00Z';
    e.effectiveAt = e.occurredAt;
  }
  return [map, middle, long];
}
