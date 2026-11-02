// Lossless lowering only: no semantic result, digest, review or case-ID logic.
export const units = s => Array.from({length:s.length}, (_,i)=>s.charCodeAt(i));
export function lower(input) {
  const nodes=[];
  const add=n=>(nodes.push(n),nodes.length-1);
  const number=n=>{const b=Buffer.alloc(8);b.writeDoubleBE(n);return add({kind:'number',bits:b.toString('hex')});};
  function value(v,construct=false) {
    if (v===null) return add({kind:'null'});
    if (typeof v==='string') return add({kind:'string',units:units(v)});
    if (typeof v==='boolean') return add({kind:'bool',value:v});
    if (typeof v==='number') return number(v);
    if (Array.isArray(v)) return add({kind:'array',items:v.map(v=>value(v,construct))});
    if (construct && Object.keys(v).length===1 && Object.hasOwn(v,'$js')) {
      switch(v.$js) {
        case '-0':return number(-0);case 'NaN':return number(NaN);case 'Infinity':return number(Infinity);
        case 'undefined':case 'bigint':case 'function':case 'symbol':case 'cycle':return add({kind:v.$js});
        case 'sparse':return add({kind:'sparse',length:2});
        case 'decorated':return add({kind:'decorated',items:[number(1)],entries:[[units('extra'),number(2)]]});
        case 'date':return add({kind:'date',iso:units('2040-01-01T00:00:00.000Z')});
        case 'null_prototype':return add({kind:'null_prototype',entries:[[units('b'),number(1)],[units('a'),number(2)]]});
        case 'symbol_key':return add({kind:'symbol_keys',entries:[[units('a'),number(2)]]});
        case 'accessor':return add({kind:'accessor',entries:[[units('a'),number(1)]]});
        default: {
          if (!/^depth_\d+$/.test(v.$js)) throw Error('s08_unknown_js_construction');
          const depth=Number(v.$js.slice(6));if(depth>90)throw Error('s08_wire_construction_depth_rejected');
          let id=add({kind:'null'});for(let i=0;i<depth;i++)id=add({kind:'array',items:[id]});return id;
        }
      }
    }
    return add({kind:'object',entries:Object.keys(v).map(k=>[units(k),value(v[k],construct)])});
  }
  const root=add({kind:'object',entries:Object.keys(input).map(k=>[units(k),value(input[k],k==='value'&&['json','knowledge','known'].includes(input.op))])});
  return {op:'semantic_v3',wire:{root,nodes},...(input.op==='review'?{ports:{uuid:'00000000-0000-4000-8000-000000000001',now:'2040-01-16T09:00:00Z'}}:{})};
}
export function requestFor(input) {return ['width','week','month'].includes(input.op)?input:lower(input);}
