// Synthetic input authoring only. Never imports the source or reads outputs.
import {writeFileSync, mkdirSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
const out = fileURLToPath(new URL('corpus/', import.meta.url));
mkdirSync(out, {recursive: true});
const cases = [];
const add = (id, vector, input, leaf = false) => cases.push({id, vector, leaf, input});
const clone = structuredClone;
const js = x => ({$js: x});
const units = s => Array.from({length: s.length}, (_, i) => s.charCodeAt(i));
for (const [id, text] of Object.entries({empty: '', ascii: 'abc', hangul: '가', emoji: '😀', bmp: '\ue000', composed: 'é', decomposed: 'e\u0301', crlf: 'a\r\nb', controls: '\0\t\x7f\x80', high: '\ud800', low: '\udfff', pair: '\ud83d\ude00', width399: 'a'.repeat(399), width400: '가'.repeat(200), width401: 'a' + '가'.repeat(200), width404: '😀'.repeat(101)})) add('width-' + id, 'EDU-17', {op: 'width', units: units(text)}, true);
for (const date of ['2026-10-18','2026-10-19','2026-10-24','2026-10-25','2026-01-01','2026-12-31','2027-01-01','2024-02-28','2024-02-29','2024-03-01','2000-01-01','2000-02-29','2099-12-31','2026-02-29','2026-02-30','2100-01-01','1999-12-31','2026-13-01','2026-01-00','2026-1-01','2026-01-01T00:00:00Z',' 2026-01-01']) add('week-' + date, 'EDU-18', {op: 'week', value: date}, true);
for (const month of ['2026-02','2026-08','2026-10','2026-11','2026-12','2027-01','2024-02','2000-01','2099-12','2026-13','2026-00','2026-1','1999-12','2100-01','2026-01-01','']) add('month-' + (month || 'empty'), 'EDU-18', {op: 'month', value: month}, true);
for (const date of ['0000-01-01','0099-12-31','1900-02-29','2000-02-29','2100-02-29','9999-12-31',null]) add('learning-date-' + date, 'EDU-18', {op: 'calendar_date', value: date});
const values = {
  ordering: {'2': 'two', '10': 'ten', '01': 'leading', z: 0, a: 1, '😀': 2, '\ue000': 3},
  insertionA: {b: 1, a: 2}, insertionB: {a: 2, b: 1},
  escapes: {'quote"\\': '"\\\b\f\n\r\t\0\u001f\u2028\u2029é e\u0301 가 😀\ud800\udfff'},
  numbers: [js('-0'), 0, -1, 0.000001, 0.0000001, 1e20, 1e21, 9007199254740991, 9007199254740992, 1.2345678901234567],
  missing: {a: 1}, null: {a: null}, blank: {a: ''}, zero: {a: 0}, unknown: {a: 'unknown'}, not_performed: {a: 'not_performed'},
  undefinedMember: {a: js('undefined'), b: null}, undefinedArray: [js('undefined')],
};
for (const family of ['learning','admission','cism']) {
  for (const [id, value] of Object.entries(values)) add('json-' + family + '-' + id, 'EDU-17', {op: 'json', family, value});
  // No recursive cycles/deep input for the unbounded admission implementation;
  // stack overflow is not a finite Terra leaf or a useful retained promise.
  for (const tag of ['undefined','NaN','Infinity','bigint','function','symbol','sparse','decorated','date','null_prototype','symbol_key','accessor', ...(family === 'learning' ? ['cycle','depth_64','depth_65'] : [])]) add('json-' + family + '-' + tag, 'EDU-17', {op: 'json', family, value: js(tag)});
}
for (const [id, value] of Object.entries({missing: js('undefined'), null: null, empty: {}, unknown: {state: 'unknown', reason: 'synthetic absent observation'}, blank: {state: 'known', value: '', evidenceRefs: ['source:synthetic']}, zero: {state: 'known', value: 0, evidenceRefs: ['source:synthetic']}, not_performed: {state: 'known', value: 'not_performed', evidenceRefs: ['teacher:synthetic']}, null_value: {state: 'known', value: null, evidenceRefs: ['source:synthetic']}, missing_value: {state: 'known', evidenceRefs: ['source:synthetic']}, missing_refs: {state: 'known', value: 0}, duplicate_refs: {state: 'known', value: 0, evidenceRefs: ['same','same']}, unknown_extra: {state: 'unknown', reason: 'absent', value: 0}})) add('knowledge-' + id, 'EDU-03', {op: 'knowledge', value});
add('known-zero-copy', 'EDU-03', {op: 'known', value: {score: 0, performed: false}, refs: ['teacher:synthetic']});
add('known-null-refused', 'EDU-03', {op: 'known', value: null, refs: ['teacher:synthetic']});
add('unknown-reason', 'EDU-03', {op: 'unknown', reason: 'No attributable assessment observed'});
add('unknown-blank-refused', 'EDU-03', {op: 'unknown', reason: ' '});
add('constants', 'EDU-17', {op: 'constants'});
const mapping = {bookLabel: 'Invented book', unitCode: '01-2', unitName: 'Invented unit', basis: 'source_edition', evidenceRefs: ['synthetic:mapping']};
const template = {lessonDate: '2040-01-16', part: 1, progress: [{content: [mapping], activity: 'solving', outcome: 'worked_ungraded', detail: 'é/e\u0301 😀', evidenceRefs: ['synthetic:activity'], internalCode: 'P1'}], homework: {dueDate: '2040-01-20', tasks: [{content: [mapping], action: 'solve', selection: '지정 3문항', evidenceRefs: ['synthetic:homework']}], qualification: 'verified'}, memo: [{text: '- - Synthetic notice. Second sentence.', parts: [1], evidenceRefs: ['synthetic:memo']}, {text: 'excluded', parts: [2], evidenceRefs: ['synthetic:excluded']}]};
const templateCase = (id, fn) => {const v = clone(template); fn(v); add('template-' + id, 'EDU-17', {op: 'template', value: v});};
templateCase('base', () => {});
templateCase('field-local-header-hold', v => {v.homework.qualification = 'unresolved_header';});
templateCase('ordered-deduplicated-content', v => {v.progress[0].content.push({...mapping}, {...mapping, unitCode: '02-1', unitName: 'Second'}, {...mapping, bookLabel: 'Other'});});
templateCase('due-first-only', v => {v.homework.tasks.push(clone(v.homework.tasks[0]));});
templateCase('part-filter', v => {v.part = 2;});
templateCase('unresolved-topic', v => {v.progress[0].content = [{...mapping, bookLabel: '', unitCode: '', basis: 'teacher_report_topic'}];});
templateCase('missing-evidence', v => {v.progress[0].evidenceRefs = [];});
templateCase('duplicate-evidence', v => {v.progress[0].evidenceRefs = ['same','same'];});
templateCase('missing-key', v => {delete v.part;});
templateCase('unknown-key', v => {v.approved = true;});
templateCase('invalid-date', v => {v.lessonDate = '2040-02-30';});
templateCase('due-before', v => {v.homework.dueDate = '2040-01-01';});
templateCase('internal-code', v => {v.progress[0].detail = 'P1_done';});
templateCase('blank-unit', v => {v.progress[0].content[0].unitCode = '';});
templateCase('coercible-enum', v => {v.progress[0].activity = ['solving'];});
templateCase('overflow-not-truncated', v => {v.memo[0].text = '가'.repeat(201);});
templateCase('empty', v => {v.progress = []; v.homework.tasks = []; v.memo = [];});
templateCase('invalid-excluded-memo-still-checked', v => {v.memo[1].evidenceRefs = [];});
const fields = ['progress','homework','lessonMemo','attendance','dailyTestAssessment','previousHomeworkAssessment'];
const scope = {sourceNamespace: 'synthetic-account', teacherId: 'teacher-fictional', studentId: 'student-fictional', lessonDate: '2040-01-16'};
const target = {...scope, courseId: 'course-fictional', membershipId: 'membership-fictional', recordId: 'current-record'};
const kv = value => ({state: 'known', value, evidenceRefs: ['synthetic:recorded']});
const observation = {scope, target, previousTarget: {...target, lessonDate: '2040-01-14', recordId: 'previous-record'}, observedAt: '2040-01-16T09:00:00.000Z', sourceRevision: '1'.repeat(64), wireDigest: '2'.repeat(64), fields: {progress: kv('- Existing progress'), homework: kv('- Existing homework'), lessonMemo: kv('- Existing notice'), attendance: kv('Y'), dailyTestAssessment: kv('8'), previousHomeworkAssessment: kv('4')}, toneSamples: [], transmissionState: 'not_sent'};
const fact = (field, text, explicitValue) => ({ref: 'fact:' + field, field, text, origin: 'user_supplied', requiredFragments: [], questionCounts: [], deadlines: [], stageClaims: [], ...(explicitValue === undefined ? {} : {explicitValue})});
const context = {schemaVersion: 'academy.dailyrecord-user-context/1', purpose: 'Synthetic independent pure review', currentRequestRef: 'synthetic-request', scope, selectedFields: fields, memoAnswer: {kind: 'content', requestRef: 'synthetic-request', text: 'Synthetic notice'}, facts: fields.map(f => fact(f, 'Synthetic fact for ' + f, f === 'attendance' ? 'N' : f === 'dailyTestAssessment' ? '0' : f === 'previousHomeworkAssessment' ? '0' : undefined))};
const proposal = (field, value, kind = 'replace') => ({field, value, factRefs: ['fact:' + field], decision: {kind, requestRef: 'synthetic-request'}});
function reviewCase(id, vector, fn) {const input = {op: 'review', observation: clone(observation), context: clone(context), proposals: [proposal('dailyTestAssessment','0'), proposal('previousHomeworkAssessment','0')], learning: {blockers: []}}; fn(input); add('review-' + id, vector, input);}
reviewCase('physical-current-and-previous', 'EDU-04', () => {});
reviewCase('missing-previous-unknown-assessment', 'EDU-04', v => {v.observation.previousTarget = null; v.observation.fields.previousHomeworkAssessment = {state: 'unknown', reasonCode: 'no_previous_record'};});
reviewCase('missing-previous-known-refusal', 'EDU-04', v => {v.observation.previousTarget = null;});
for (const [key, value] of Object.entries({studentId:'foreign', recordId:'current-record', lessonDate:'2040-01-16', courseId:'foreign', membershipId:'foreign', sourceNamespace:'foreign'})) reviewCase('previous-conflict-' + key, 'EDU-04', v => {v.observation.previousTarget[key] = value;});
reviewCase('learning-hold-independent-assessment', 'EDU-05', v => {v.proposals = [proposal('progress','- New mapped text'), proposal('attendance','N')]; v.learning.blockers = ['student_namespace_bridge_unverified_ask_teacher'];});
reviewCase('missing-learning-policy', 'EDU-05', v => {v.proposals = [proposal('progress','- New text'), proposal('attendance','N')]; v.learning = null;});
for (const kind of ['set','replace','clear']) reviewCase('narrative-' + kind, 'EDU-06', v => {v.proposals = [proposal('progress',kind === 'clear' ? '' : '- New text',kind)];});
reviewCase('narrative-noop-set', 'EDU-06', v => {v.proposals = [proposal('progress','- Existing progress','set')];});
reviewCase('memo-no-addition-clear-hold', 'EDU-06', v => {v.context.memoAnswer = {kind:'no_addition',requestRef:'synthetic-request'}; v.proposals = [proposal('lessonMemo','','clear')];});
reviewCase('unsupported-completion', 'EDU-06', v => {v.proposals = [proposal('progress','- 풀이 완료')];});
reviewCase('teacher-preserve-before', 'EDU-06', v => {v.context.facts[0].text = 'Exact synthetic report'; v.proposals = [{...proposal('progress','- Existing progress\n- Exact synthetic report'), teacherReported:{preserveBefore:true,mappedText:'',lines:[{factRef:'fact:progress',text:'- Exact synthetic report'}]}}];});
for (const value of ['-1','0','']) reviewCase('assessment-before-' + (value || 'blank'), 'EDU-03', v => {v.observation.fields.dailyTestAssessment = kv(value); v.proposals = [proposal('dailyTestAssessment','0','set')];});
reviewCase('assessment-unknown', 'EDU-03', v => {v.observation.fields.dailyTestAssessment = {state:'unknown',reasonCode:'not_observed'};});
reviewCase('assessment-missing', 'EDU-03', v => {delete v.observation.fields.dailyTestAssessment;});
reviewCase('unavailable', 'EDU-03', v => {v.observation = null;});
const httpTarget = {studentName:'Fictional Student',date:'2040-01-16',groupId:'90001',teacherKey:'90002',studentKey:'90003',courseKey:'90004',recordKey:'90005',membershipKey:'90006'};
const state = {target:httpTarget,fields:{progress:'old',homework:'old',lessonMemo:'old',attendance:'Y',dailyTestAssessment:'8',previousHomeworkAssessment:'4'},previousRecordKey:'90007',previousDate:'2040-01-14',wireDigest:'2'.repeat(64)};
for (const field of ['dailyTestAssessment','previousHomeworkAssessment']) for (const correct of [true,false]) {
 const action = {attemptId:'synthetic-attempt',target:httpTarget,field,effectRecordKey:correct && field==='previousHomeworkAssessment'?'90007':'90005',before:state.fields[field],after:'0',wireDigest:state.wireDigest};
 if (!correct && field==='dailyTestAssessment') action.effectRecordKey = '90007';
 add('http-' + field + '-' + correct, 'EDU-04', {op:'http_action',action,state});
}
writeFileSync(out + 'cases.json', JSON.stringify(cases, null, 2) + '\n', {flag:'wx'});
console.log(JSON.stringify({cases:cases.length,leaf:cases.filter(c=>c.leaf).length}));
