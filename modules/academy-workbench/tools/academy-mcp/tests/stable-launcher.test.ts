import {expect,test} from 'bun:test';
import {mkdtemp,mkdir,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {createHash} from 'node:crypto';

test('stable launcher follows V1 release generation only with its bridge pin',async()=>{
 const base=await mkdtemp(join(tmpdir(),'academy-launcher-')),release=join(base,'releases','synthetic-one');
 const launcher=resolve(import.meta.dir,'../src/stable-launcher.ts');
 const run=async()=>{const p=Bun.spawn([process.execPath,launcher],{env:{...process.env,ACADEMY_MCP_INSTALL_DIR:base},stdout:'pipe',stderr:'pipe'});return {code:await p.exited,out:await new Response(p.stdout).text(),err:await new Response(p.stderr).text()};};
 try{
  await mkdir(release,{recursive:true});const bridge=join(release,'server.mjs'),source='process.stdout.write("synthetic bridge\\n");';
  await writeFile(bridge,source,{mode:0o600});await writeFile(join(release,'bridge-pin.json'),JSON.stringify({path:bridge,sha256:createHash('sha256').update(source).digest('hex')}),{mode:0o600});
  await writeFile(join(base,'release.json'),JSON.stringify({schemaVersion:'academy.mcp-release/1',generation:'synthetic-one'}),{mode:0o600});
  expect(await run()).toMatchObject({code:0,out:'synthetic bridge\n'});
  await writeFile(bridge,'process.stdout.write("tampered");');const failed=await run();expect(failed.code).toBe(1);expect(failed.err).toContain('launcher_bridge_hash_mismatch');expect(failed.out).toBe('');
 }finally{await rm(base,{recursive:true,force:true});}
});
