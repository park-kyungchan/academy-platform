import{test}from'node:test';import assert from'node:assert/strict';import{parseClassDays,shouldAttemptLiveRead}from'../src/class-day-guard.ts';
test('class names resolve to weekday numbers',()=>{assert.deepEqual(parseClassDays('월수1반'),[1,3]);assert.deepEqual(parseClassDays('월금1반'),[1,5]);assert.deepEqual(parseClassDays('화목'),[2,4]);});
test('duplicates collapse and order is canonical',()=>{assert.deepEqual(parseClassDays('수월수'),[1,3]);});
test('unknown or empty class names fail closed',()=>{for(const name of ['','1반','월수X','A'])assert.throws(()=>parseClassDays(name as string),/invalid_class_name/);});
test('non-class day skips with evidence-attributed reason',()=>{const r=shouldAttemptLiveRead({classDays:[1,3],lessonDate:'2026-10-23',basis:'user 2026-10-26: 신지우=월수1반'});assert.equal(r.attempt,false);assert.match(r.reason,/non_class_day_skip/);assert.match(r.reason,/2026-10-23/);assert.match(r.reason,/신지우=월수1반/);});
test('class day proceeds',()=>{const r=shouldAttemptLiveRead({classDays:[1,3],lessonDate:'2026-10-26',basis:'user 2026-10-26: 신지우=월수1반'});assert.equal(r.attempt,true);});
test('bad dates and empty class days fail closed',()=>{assert.throws(()=>shouldAttemptLiveRead({classDays:[1,3],lessonDate:'2026-10-32',basis:'x'}),/invalid_lesson_date/);assert.throws(()=>shouldAttemptLiveRead({classDays:[],lessonDate:'2026-10-26',basis:'x'}),/invalid_class_days/);});
