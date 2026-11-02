import {isDeepStrictEqual} from 'node:util';
const own=(v,k)=>v!==null&&typeof v==='object'&&Object.hasOwn(v,k);
export function frameValid(v){
 if(!v||typeof v!=='object'||Array.isArray(v))return false;
 if(!isDeepStrictEqual(Object.keys(v).sort(),['effects','events','metadata','outcome']))return false;
 if(!v.effects||typeof v.effects!=='object'||!isDeepStrictEqual(Object.keys(v.effects).sort(),['deniedTransportAttempts','forbiddenFakeDispatches']))return false;
 if(v.effects.deniedTransportAttempts!==0||v.effects.forbiddenFakeDispatches!==0)return false;
 if(!Array.isArray(v.events)||!v.metadata||!Array.isArray(v.metadata.sourceCalls)||!v.metadata.sourceCalls.every(s=>typeof s==='string')||!Array.isArray(v.metadata.uuidValues)||!v.metadata.uuidValues.every(s=>typeof s==='string'))return false;
 const o=v.outcome;
 return !!o&&typeof o==='object'&&((o.kind==='return'&&own(o,'value')&&isDeepStrictEqual(Object.keys(o).sort(),['kind','value']))||(o.kind==='throw'&&o.error&&typeof o.error.name==='string'&&typeof o.error.message==='string'&&(!own(o.error,'issues')||Array.isArray(o.error.issues))&&isDeepStrictEqual(Object.keys(o).sort(),['error','kind'])));
}
export function judge(expected,actual){
 if(!actual||typeof actual.status!=='number'||actual.status!==0||actual.signal!==null||actual.spawnError!==null)return {accepted:false,reason:'child_status'};
 if(typeof actual.stdout!=='string'||actual.stderr!=='')return {accepted:false,reason:'stream_type_or_stderr'};
 let value;try{value=JSON.parse(actual.stdout);}catch{return {accepted:false,reason:'stdout_not_json'};}
 // Canonical single JSON line also catches duplicate keys, BOM, trailing junk,
 // whitespace, noncanonical numeric spellings and multiple protocol frames.
 if(actual.stdout!==JSON.stringify(value)+'\n')return {accepted:false,reason:'framing_or_duplicate_keys'};
 if(!frameValid(value)||!frameValid(expected))return {accepted:false,reason:'frame_or_effect_contract'};
 return isDeepStrictEqual(value,expected)?{accepted:true,reason:'full_frame'}:{accepted:false,reason:'value_type_presence_order_or_bytes'};
}
export function judgeSuite(cases,expected,actual){
 const ids=cases.map(c=>c.id);
 if(new Set(ids).size!==ids.length||!isDeepStrictEqual(Object.keys(expected),ids)||!Array.isArray(actual)||!isDeepStrictEqual(actual.map(c=>c.id),ids))return {accepted:false,reason:'incomplete_duplicate_or_reordered_scope'};
 const comparisons=actual.map(c=>({id:c.id,...judge(expected[c.id],c.actual)}));
 return {accepted:comparisons.every(c=>c.accepted),comparisons};
}
