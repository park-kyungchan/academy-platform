import {test} from 'node:test';import assert from 'node:assert/strict';
import {lower,requestFor,units} from './wire.mjs';
test('UTF16 strings and keys never use scalar iteration',()=>{
  const a=lower({op:'json',family:'cism',value:{'😀\ud800':'é e\u0301 😀\udfff'}});
  const object=a.wire.nodes.find(n=>n.kind==='object'&&n.entries[0]?.[0]?.[0]===0xd83d);
  assert.deepEqual(object.entries[0][0],[0xd83d,0xde00,0xd800]);
  assert.deepEqual(a.wire.nodes[object.entries[0][1]].units,units('é e\u0301 😀\udfff'));
});
test('binary64, negative zero and nonfinite construction remain explicit',()=>{
  for(const [value,bits] of [[{$js:'-0'},'8000000000000000'],[{$js:'Infinity'},'7ff0000000000000'],[1e21,'444b1ae4d6e2ef50']]){
    const r=lower({op:'json',family:'learning',value});assert.ok(r.wire.nodes.some(n=>n.kind==='number'&&n.bits===bits));
  }
});
test('property order and distinct object descriptors survive lowering',()=>{
  for(const [tag,kind] of [['sparse','sparse'],['decorated','decorated'],['null_prototype','null_prototype'],['symbol_key','symbol_keys'],['accessor','accessor'],['cycle','cycle']]){
    assert.ok(lower({op:'json',family:'learning',value:{$js:tag}}).wire.nodes.some(n=>n.kind===kind));
  }
  const a=lower({op:'knowledge',value:{b:1,a:2}});assert.deepEqual(a.wire.nodes.find(n=>n.kind==='object'&&n.entries.length===2).entries.map(([k])=>String.fromCharCode(...k)),['b','a']);
});
test('construction is scoped to original materialize arguments, never business fields',()=>{
  const r=lower({op:'review',context:{value:{$js:'undefined'}}});assert.ok(!r.wire.nodes.some(n=>n.kind==='undefined'));
  assert.equal(r.ports.uuid,'00000000-0000-4000-8000-000000000001');assert.equal(r.ports.now,'2040-01-16T09:00:00Z');
});
test('leaf protocol preserved; depth65 is flat and retained',()=>{
  const a={op:'width',units:[0xd800]};assert.equal(requestFor(a),a);
  assert.equal(lower({op:'json',family:'learning',value:{$js:'depth_65'}}).wire.nodes.filter(n=>n.kind==='array').length,65);
});
