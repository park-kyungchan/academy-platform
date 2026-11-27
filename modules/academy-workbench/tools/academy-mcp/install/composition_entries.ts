// Prints the registry entries of the bridge's compositions exactly as `neutral-server.ts --discover` writes them,
// from the module source of the checkout this script lies in (the top of its Git work tree), without starting any endpoint.
// Usage: bun <checkout>/modules/academy-workbench/tools/academy-mcp/install/composition_entries.ts > entries.json
// Used by install/registry_surface_install.py --compositions <entries.json>: an entry replaces the registry entry
// of its name, or is added when the registry has none (the consolidated tools of src/consolidated-tools.ts).
const usage='usage: bun <checkout>/modules/academy-workbench/tools/academy-mcp/install/composition_entries.ts > <entries.json>';
if(process.argv.length>2){console.error(usage);process.exit(2);}
const top=Bun.spawnSync(['git','-C',import.meta.dir,'rev-parse','--show-toplevel']);
if(!top.success)throw Error('checkout_not_found: '+top.stderr.toString().trim());
const mod=top.stdout.toString().trim()+'/modules/academy-workbench/tools/academy-mcp';
const {compositionEntries} = await import(mod + '/src/consolidated-tools.ts');
console.log(JSON.stringify(compositionEntries(), null, 2));
