// Prints the registry entries of the bridge's two compositions exactly as `neutral-server.ts --discover` writes them,
// from the module source of the checkout this script lies in (the top of its Git work tree), without starting any endpoint.
// Usage: bun <checkout>/modules/academy-workbench/tools/academy-mcp/install/composition_entries.ts > entries.json
// Used by install/registry_surface_install.py --compositions <entries.json> when a merged change alters their schemas.
const usage='usage: bun <checkout>/modules/academy-workbench/tools/academy-mcp/install/composition_entries.ts > <entries.json>';
if(process.argv.length>2){console.error(usage);process.exit(2);}
const top=Bun.spawnSync(['git','-C',import.meta.dir,'rev-parse','--show-toplevel']);
if(!top.success)throw Error('checkout_not_found: '+top.stderr.toString().trim());
const mod=top.stdout.toString().trim()+'/modules/academy-workbench/tools/academy-mcp';
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
