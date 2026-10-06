import { expect, test } from 'bun:test';
import { createHash } from 'node:crypto';
import { validateSchema } from '../scripts/schema-check.ts';
import { deriveAttention, historyEventTime } from '../src/domain/book-flow-attention.ts';
import type { RequirementFacts, HistoryEvent, BookFlowStore } from '../src/contracts/book-flow.ts';
import { SECTION_REGISTRY } from '../src/contracts/curriculum.ts';
import { createCurriculumFixture } from '../fixtures/curriculum.ts';
import { currentEvidence, requirementAttention, requirementState, progressSummary } from '../src/domain/curriculum.ts';
import { curriculumView } from '../src/ui/curriculum-view.ts';

const root = new URL('../imports/academy-contract-r2/', import.meta.url);
const schema = await Bun.file(new URL('academy-book-flow-1.0.schema.json',root)).json();
const contract = await Bun.file(new URL('academy-book-flow-contract.r2.json',root)).json();
type Example = { id:string; synthetic:boolean; input:RequirementFacts; expected:{confirmedMissing:boolean; confirmedNeverStarted:boolean; badgesInclude:string[]; mustNotFlagMissingHomeApp?:boolean; includeInRequiredDenominator?:boolean} };
const fixtures: { allSynthetic:boolean; personalDataIncluded:boolean; cases:Example[] } = await Bun.file(new URL('book-flow.semantic-fixtures.json',root)).json();
const context = {factIds:['synthetic-fact'],coverageIds:['synthetic-coverage'],conflictIds:[],asOf:'2026-10-06T08:00:00Z',derivedAt:'2026-10-06T08:00:00Z'};

for (const example of fixtures.cases) test(`r2 semantic ${example.id}`,()=>{
  expect(example.synthetic).toBe(true);
  expect(validateSchema(example.input,schema.$defs.RequirementFacts,schema)).toEqual([]);
  const before=structuredClone(example.input), result=deriveAttention(example.input,context);
  expect(result.confirmedMissing).toBe(example.expected.confirmedMissing);
  expect(result.confirmedNeverStarted).toBe(example.expected.confirmedNeverStarted);
  for(const badge of example.expected.badgesInclude) expect(result.badges).toContain(badge);
  if(example.expected.mustNotFlagMissingHomeApp) expect(result.flagMissingHomeApp).toBe(false);
  if(example.expected.includeInRequiredDenominator!==undefined) expect(result.includeInRequiredDenominator).toBe(example.expected.includeInRequiredDenominator);
  expect(example.input).toEqual(before);
});

test('r2 pack integrity and contract/schema version are independently verified',async()=>{
  const manifest=await Bun.file(new URL('../docs/r2-import-manifest.json',import.meta.url)).json();
  for(const file of manifest.files) expect(createHash('sha256').update(new Uint8Array(await Bun.file(new URL(file.name,root)).arrayBuffer())).digest('hex')).toBe(file.sha256);
  expect(validateSchema(contract,schema)).toEqual([]);
  expect(contract.schemaVersion).toBe('1.0.0'); expect(contract.revision).toBe(2);
  expect(fixtures.cases).toHaveLength(10);expect(fixtures.allSynthetic).toBe(true);expect(fixtures.personalDataIncluded).toBe(false);
  expect(contract.sectionProfiles[0].definitions.map((d:{label:string})=>d.label)).toEqual([...SECTION_REGISTRY.elementary]);
  expect(contract.sectionProfiles[1].definitions.map((d:{label:string})=>d.label)).toEqual([...SECTION_REGISTRY.middle]);
});
test('r2 schema rejects invalid types unknown keys dates and non-local references',()=>{
  const facts=fixtures.cases[0]!.input;
  for(const bad of [{...facts,completion:'done'},{...facts,unexpected:true},{...facts,dueDate:'2026-02-30'},{...facts,dueAt:'2026-10-06T08:00:00'}]) expect(validateSchema(bad,schema.$defs.RequirementFacts,schema).length).toBeGreaterThan(0);
  expect(()=>validateSchema({},{$ref:'https://invalid.example/schema'})).toThrow('Only local');
  expect(()=>validateSchema({}, {unknownKeyword:true})).toThrow('Unsupported');
  expect(validateSchema('bad',{oneOf:[{type:'number'},{type:'null'}]}).length).toBeGreaterThan(0);
  expect(validateSchema({kind:'video',section:'invented'},{if:{properties:{kind:{const:'video'}}},then:{properties:{section:{type:'null'}}}}).length).toBeGreaterThan(0);
});
test('r2 UI projection separates incomplete from never-started and retains coverage badges',()=>{
  const r=createCurriculumFixture('synthetic-student-1')[0]!.requirements.find(r=>r.id==='s1-problem')!;
  r.axes.effort='started_confirmed';r.axes.coverage='blocked';
  expect(requirementState(r)).toBe('incomplete');
  expect(requirementAttention(r)).toMatchObject({confirmedMissing:true,confirmedNeverStarted:false});
  expect(requirementAttention(r).badges).toContain('조회 제한');
  r.axes.requiredness='optional';expect(requirementAttention(r).confirmedMissing).toBe(false);
});
test('r2 event time never falls back to import time and incomplete hierarchy suppresses totals',()=>{
  const event:HistoryEvent={id:'synthetic-event',schemaVersion:'1.0.0',revision:1,kind:'source_observed',target:{studentId:'synthetic-student',bookEditionId:null,nodeId:null,assignmentId:null,requirementId:null,classSessionId:null},occurredAt:null,effectiveAt:null,observedAt:'2026-10-06T09:00:00Z',recordedAt:'2026-10-06T10:00:00Z',sourceEventTimezone:null,payloadReference:{entityId:'synthetic-observation',entityKind:'Observation',schemaVersion:'1.0.0',revision:1,contentHash:null},provenance:{sourceIds:[],coverageIds:[]},supersedesEventIds:[],correctionReason:null,factState:'observed',synthetic:true};
  expect(validateSchema(event,schema.$defs.HistoryEvent,schema)).toEqual([]);
  expect(historyEventTime(event)).toBeNull();
  expect(historyEventTime({...event,occurredAt:'2026-10-01T01:00:00Z'})).toBe('2026-10-01T01:00:00Z');
  const map=createCurriculumFixture('synthetic-student-1')[0]!;map.requirements=[map.requirements[0]!];
  expect(progressSummary(map).percent).toBeNull();
  const r=map.requirements[0]!;r.evidence[0]!.effectiveAt=null;r.evidence[0]!.occurredAt=null;
  expect(currentEvidence(r)).toEqual([]);expect(requirementState(r)).toBe('unresolved');
});
test('r2 store capability contract permits unsupported atomicity without claiming an implementation',()=>{
  const capabilities:BookFlowStore['capabilities']={atomicMultiRecordCommit:'unsupported',conditionalWrites:'unknown',nativeRelationalQueries:'unsupported'};
  expect(capabilities.atomicMultiRecordCommit).toBe('unsupported');
  expect(contract.storageAdapter.relationalDatabaseClaim).toBe(false);
  expect(contract.dataLogicAction.webMcp.runtimeExecutionImplemented).toBe(false);
  expect(contract.dataLogicAction.audit.actualEvents).toEqual([]);
});
test('r2 rendered evidence preserves simultaneous incomplete and blocked badges',()=>{
  const map=createCurriculumFixture('synthetic-student-1')[0]!;
  const r=map.requirements.find(r=>r.id==='s1-problem')!;r.axes.effort='started_confirmed';r.axes.coverage='blocked';
  const html=curriculumView(map,'가상 학생 별',new URLSearchParams({student:map.studentId,book:map.book.id,section:'s1'}));
  expect(html).toContain('attention-badges');expect(html).toContain('확정 미완료');expect(html).toContain('조회 제한');expect(html).toContain('value="incomplete"');
});
