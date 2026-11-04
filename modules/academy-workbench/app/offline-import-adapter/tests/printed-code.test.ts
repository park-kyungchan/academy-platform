import{test}from'node:test';import assert from'node:assert/strict';import{printedCodeToNumber,numberToPrintedCode}from'../src/printed-code.ts';
test('verified paper codes decode to their paper numbers',()=>{assert.equal(printedCodeToNumber('#8E9RN'),6692596);assert.equal(printedCodeToNumber('#839RN'),6332148);});
test('paper numbers encode back to their printed codes',()=>{assert.equal(numberToPrintedCode(6692596),'#8E9RN');assert.equal(numberToPrintedCode(6332148),'#839RN');});
test('decode uppercases lowercase input',()=>{assert.equal(printedCodeToNumber('#8e9rn'),6692596);});
test('encode then decode roundtrips',()=>{for(const n of [1,31,32,33,6692596,6332148,4294967295])assert.equal(printedCodeToNumber(numberToPrintedCode(n)),n);});
test('invalid codes fail closed',()=>{for(const code of ['','#','8E9RN','#8E9R!','#8E9RI','#8E9R0','#8E9R1','#8E9R ','#8E9RNXXXXXXXX'])assert.throws(()=>printedCodeToNumber(code),/invalid_printed_code/);});
test('invalid numbers fail closed',()=>{for(const n of [-1,1.5,NaN,4294967296])assert.throws(()=>numberToPrintedCode(n as number),/invalid_paper_number/);});
