// Prints the registry entries of the bridge's two compositions exactly as `neutral-server.ts --discover` writes them,
// from the module source of a checkout, without starting any endpoint.
// Usage: bun harness/composition_entries.ts <module dir: .../modules/academy-workbench/tools/academy-mcp> > entries.json
// Used by harness/registry_surface_install.py --compositions <entries.json> when a merged change alters their schemas.
const mod = process.argv[2];
if (!mod) throw Error('usage: composition_entries.ts <module dir>');
const {z} = await import(mod + '/node_modules/zod/index.js');
const {dailyRecordGetSchema, dailyRecordGetDescription} = await import(mod + '/src/dailyrecord-read.ts');
const {dailyTestHeaderRouteSchema, dailyTestHeaderRouteDescription} = await import(mod + '/src/dailytest-header-route.ts');
const {readOnlyAnnotations} = await import(mod + '/src/contracts.ts');
const entries = [
  {endpoint: 'dailyrecord', upstreamName: 'academy_dailyrecord_get', tool: {name: 'academy_dailyrecord_get',
    title: 'Read live DailyRecord by student name (today by default)', description: dailyRecordGetDescription,
    inputSchema: z.toJSONSchema(dailyRecordGetSchema, {io: 'input'}), annotations: {...readOnlyAnnotations, openWorldHint: true, idempotentHint: false}}},
  {endpoint: 'dailyrecord', upstreamName: 'academy_dailytest_history_with_headers', tool: {name: 'academy_dailytest_history_with_headers',
    description: dailyTestHeaderRouteDescription, inputSchema: z.toJSONSchema(dailyTestHeaderRouteSchema, {io: 'input'}),
    annotations: {...readOnlyAnnotations, openWorldHint: true, idempotentHint: false}}},
];
console.log(JSON.stringify(entries, null, 2));
