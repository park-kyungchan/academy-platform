import {expect,test} from 'bun:test';
import {mkdtempSync,mkdirSync,writeFileSync,symlinkSync,unlinkSync,readFileSync,rmSync} from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
test('private JSON descriptors refuse replacement instead of following an output symlink',async()=>{
 const module=await import('../../harness/academy_private_io').catch(()=>({reservePrivateJson:undefined}));expect(typeof module.reservePrivateJson).toBe('function');
 const home=mkdtempSync(join(tmpdir(),'journal-io-'));mkdirSync(join(home,'artifacts'));const path=join(home,'artifacts','out.json'),foreign=join(home,'foreign.json');writeFileSync(foreign,'preserve');
 const result=module.reservePrivateJson!(home,path);try{result.write({state:'started'});unlinkSync(path);symlinkSync(foreign,path);expect(()=>result.write({state:'late'})).toThrow('private_json_target_changed');expect(readFileSync(foreign,'utf8')).toBe('preserve');}finally{result.close();rmSync(home,{recursive:true,force:true});}
});
