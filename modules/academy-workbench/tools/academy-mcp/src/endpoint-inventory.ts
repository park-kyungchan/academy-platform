/** Validate names against the pinned runtime registry before any source read or possible write. */
export class EndpointInventoryMismatch extends Error {
 constructor(readonly endpoint:string,readonly missingTools:readonly string[]){super('upstream_tool_inventory_mismatch');}
}
export function verifyEndpointInventory(endpoint:string,registry:readonly {endpoint:string;upstreamName:string}[],listed:readonly {name:string}[]){
 const compositions=new Set(['academy_dailytest_history_with_headers','academy_dailyrecord_get']);
 const expected=registry.filter(r=>r.endpoint===endpoint&&!compositions.has(r.upstreamName));
 const names=new Set(listed.map(t=>t.name));
 const missing=[...new Set(expected.map(r=>r.upstreamName))].filter(n=>!names.has(n)).sort();
 if(missing.length)throw new EndpointInventoryMismatch(endpoint,missing);
 return {endpoint,expectedTools:new Set(expected.map(r=>r.upstreamName)).size,listedTools:names.size};
}
